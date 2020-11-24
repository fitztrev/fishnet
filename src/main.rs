mod configure;
mod assets;
mod systemd;
mod api;
mod ipc;
mod queue;
mod util;

use tracing::{debug, info, error};
use tokio::signal;
use tokio::sync::{mpsc, oneshot};
use crate::configure::{Opt, Command, Cores};
use crate::assets::Cpu;
use crate::ipc::{Pull, PositionResponse};

#[tokio::main]
async fn main() {
    let opt = configure::parse_and_configure().await;

    if opt.auto_update {
        todo!("--auto-update");
    }

    match opt.command {
        Some(Command::Run) | None => run(opt).await,
        Some(Command::Systemd) => systemd::systemd_system(opt),
        Some(Command::SystemdUser) => systemd::systemd_user(opt),
        Some(Command::Configure) => (),
    }
}

async fn run(opt: Opt) {
    let cpu = Cpu::detect();
    info!("CPU features: {:?}", cpu);

    let cores = usize::from(opt.cores.unwrap_or(Cores::Auto));
    info!("Cores: {}", cores);

    // Install handler for SIGTERM.
    #[cfg(unix)]
    let mut sig_term = signal::unix::signal(signal::unix::SignalKind::terminate()).expect("install handler for sigterm");
    #[cfg(not(unix))]
    let mut sig_term = signal::windows::ctrl_break().expect("install handler for ctrl+break");

    // Install handler for SIGINT.
    #[cfg(unix)]
    let mut sig_int = signal::unix::signal(signal::unix::SignalKind::interrupt()).expect("install handler for sigint");
    #[cfg(not(unix))]
    let mut sig_int = signal::windows::ctrl_c().expect("install handler for ctrl+c");

    // To wait for workers and API actor before shutdown.
    let mut join_handles = Vec::new();

    // Spawn API actor.
    let endpoint = opt.endpoint();
    info!("Endpoint: {}", endpoint);
    let api = {
        let (api, api_actor) = api::channel(endpoint, opt.key);
        join_handles.push(tokio::spawn(async move {
            api_actor.run().await;
        }));
        api
    };

    // Spawn queue actor.
    let mut queue = {
        let (queue, queue_actor) = queue::channel(opt.backlog, api);
        join_handles.push(tokio::spawn(async move {
            queue_actor.run().await;
        }));
        queue
    };

    // Spawn workers. Workers handle engine processes and send their results
    // to tx, thereby requesting more work.
    let mut rx = {
        let (tx, rx) = mpsc::channel::<Pull>(cores);
        for i in 0..cores {
            let tx = tx.clone();
            join_handles.push(tokio::spawn(async move {
                debug!("Started worker {}.", i);

                let mut job: Option<ipc::Position> = None;

                loop {
                    let response = if let Some(job) = job.take() {
                        debug!("Working on {:?}", job);
                        let t = std::time::Duration::from_secs(5);
                        tokio::time::sleep(t).await;
                        Some(PositionResponse {
                            batch_id: job.batch_id,
                            position_id: job.position_id,
                            score: crate::ipc::Score::Cp(50),
                            best_move: None,
                            pv: Vec::new(),
                            depth: 20,
                            nodes: 3_500_000,
                            time: t,
                            nps: Some(3_500_000 / 5),
                        })
                    } else {
                        None
                    };

                    let (callback, waiter) = oneshot::channel();

                    if let Err(_) = tx.send(Pull {
                        response,
                        callback,
                    }).await {
                        error!("Worker was about to send result, but tx is dead.");
                        break;
                    }

                    tokio::select! {
                        _ = tx.closed() => break,
                        res = waiter => {
                            match res {
                                Ok(j) => job = Some(j),
                                Err(_) => break,
                            }
                        }
                    }
                }

                debug!("Stopped worker {}.", i);
                drop(tx);
            }));
        }
        rx
    };

    let mut shutdown_soon = false;

    // Main loop. Handles signals, forwards worker results from rx to the HTTP
    // API and responds with more work.
    loop {
        tokio::select! {
            res = sig_int.recv() => {
                res.expect("sigint handler installed");
                if shutdown_soon {
                    info!("Stopping now.");
                    rx.close();
                } else {
                    info!("Stopping soon. Press ^C again to abort pending jobs ...");
                    queue.shutdown_soon().await;
                    shutdown_soon = true;
                }
            }
            res = sig_term.recv() => {
                res.expect("sigterm handler installed");
                info!("Stopping now.");
                shutdown_soon = true;
                rx.close();
            }
            res = rx.recv() => {
                if let Some(res) = res {
                    debug!("Forwarding pull.");
                    queue.pull(res).await;
                } else {
                    debug!("All workers dropped their tx.");
                    break;
                }
            }
        }
    }

    // Shutdown queue to abort remaining jobs.
    queue.shutdown().await;

    debug!("Bye.");
    for join_handle in join_handles.into_iter() {
        join_handle.await.expect("join");
    }
}
