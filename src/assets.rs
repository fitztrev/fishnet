use std::fmt;
use std::io;
use std::fs::File;
use std::path::{Path, PathBuf};
use bitflags::bitflags;
use tempfile::TempDir;
use xz::read::XzDecoder;

struct Asset {
    name: &'static str,
    data: &'static [u8],
    needs: Cpu,
    executable: bool,
}

impl Asset {
    #[cfg(unix)]
    fn open_file(&self, path: &Path) -> io::Result<File> {
        use std::os::unix::fs::OpenOptionsExt as _;
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .mode(0o700)
            .open(path)
    }

    #[cfg(not(unix))]
    fn open_file(&self, path: &Path) -> io::Result<File> {
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(path)
    }

    fn create(&self, base: &Path) -> io::Result<PathBuf> {
        let path = base.join(self.name);
        let mut file = self.open_file(&path)?;

        let mut decoder = XzDecoder::new(self.data);
        io::copy(&mut decoder, &mut file)?;

        file.sync_all()?;
        Ok(path)
    }
}

impl fmt::Debug for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Asset")
            .field("name", &self.name)
            .field("needs", &self.needs)
            .field("executable", &self.executable)
            .field("data", &"..")
            .finish()
    }
}

bitflags! {
    pub struct Cpu: u32 {
        const POPCNT = 1 << 0;
        const SSE    = 1 << 1;
        const SSE2   = 1 << 2;
        const SSSE3  = 1 << 3;
        const SSE41  = 1 << 4;
        const AVX2   = 1 << 5;
        const BMI2   = 1 << 6;
        const INTEL  = 1 << 7; // amd supports bmi2, but pext is too slow

        const SF              = 0;
        const SF_SSSE3        = Cpu::SF.bits | Cpu::SSE.bits | Cpu::SSE2.bits | Cpu::SSSE3.bits;
        const SF_SSE41_POPCNT = Cpu::SF_SSSE3.bits | Cpu::POPCNT.bits | Cpu::SSE41.bits;
        const SF_AVX2         = Cpu::SF_SSE41_POPCNT.bits | Cpu::AVX2.bits;
        const SF_BMI2         = Cpu::SF_AVX2.bits | Cpu::BMI2.bits | Cpu::INTEL.bits;
    }
}

impl Cpu {
    #[cfg(target_arch = "x86_64")]
    pub fn detect() -> Cpu {
        let mut cpu = Cpu::empty();
        cpu.set(Cpu::POPCNT, is_x86_feature_detected!("popcnt"));
        cpu.set(Cpu::SSE, is_x86_feature_detected!("sse"));
        cpu.set(Cpu::SSE2, is_x86_feature_detected!("sse"));
        cpu.set(Cpu::SSSE3, is_x86_feature_detected!("ssse3"));
        cpu.set(Cpu::SSE41, is_x86_feature_detected!("sse4.1"));
        cpu.set(Cpu::AVX2, is_x86_feature_detected!("avx2"));
        cpu.set(Cpu::BMI2, is_x86_feature_detected!("bmi2"));

        cpu.set(Cpu::INTEL, match raw_cpuid::CpuId::new().get_vendor_info() {
            Some(vendor) => vendor.as_string() == "GenuineIntel",
            None => false,
        });

        cpu
    }

    #[cfg(not(target_arch = "x86_64"))]
    pub fn detect() -> Cpu {
        Cpu::empty()
    }
}

const NNUE: Asset = Asset {
    name: "nn-c3ca321c51c9.nnue",
    data: include_bytes!("../assets/nn-c3ca321c51c9.nnue.xz"),
    needs: Cpu::empty(),
    executable: false,
};

#[cfg(all(unix, target_arch = "x86_64", not(target_os = "macos")))]
const STOCKFISH: &'static [Asset] = &[
    Asset {
        name: "stockfish-x86-64-bmi2",
        data: include_bytes!("../assets/stockfish-x86-64-bmi2.xz"),
        needs: Cpu::SF_BMI2,
        executable: true,
    },
    Asset {
        name: "stockfish-x86-64-avx2",
        data: include_bytes!("../assets/stockfish-x86-64-avx2.xz"),
        needs: Cpu::SF_AVX2,
        executable: true,
    },
    Asset {
        name: "stockfish-x86-64-sse41-popcnt",
        data: include_bytes!("../assets/stockfish-x86-64-sse41-popcnt.xz"),
        needs: Cpu::SF_SSE41_POPCNT,
        executable: true,
    },
    Asset {
        name: "stockfish-x86-64-ssse3",
        data: include_bytes!("../assets/stockfish-x86-64-ssse3.xz"),
        needs: Cpu::SF_SSSE3,
        executable: true,
    },
    Asset {
        name: "stockfish-x86-64",
        data: include_bytes!("../assets/stockfish-x86-64.xz"),
        needs: Cpu::SF,
        executable: true,
    },
];

#[cfg(all(unix, target_arch = "x86_64", not(target_os = "macos")))]
const STOCKFISH_MV: &'static [Asset] = &[
    Asset {
        name: "stockfish-mv-x86-64-bmi2",
        data: include_bytes!("../assets/stockfish-mv-x86-64-bmi2.xz"),
        needs: Cpu::SF_BMI2,
        executable: true,
    },
    Asset {
        name: "stockfish-mv-x86-64-avx2",
        data: include_bytes!("../assets/stockfish-mv-x86-64-avx2.xz"),
        needs: Cpu::SF_AVX2,
        executable: true,
    },
    Asset {
        name: "stockfish-mv-x86-64-sse41-popcnt",
        data: include_bytes!("../assets/stockfish-mv-x86-64-sse41-popcnt.xz"),
        needs: Cpu::SF_SSE41_POPCNT,
        executable: true,
    },
    Asset {
        name: "stockfish-mv-x86-64-ssse3",
        data: include_bytes!("../assets/stockfish-mv-x86-64-ssse3.xz"),
        needs: Cpu::SF_SSSE3,
        executable: true,
    },
    Asset {
        name: "stockfish-mv-x86-64",
        data: include_bytes!("../assets/stockfish-mv-x86-64.xz"),
        needs: Cpu::SF,
        executable: true,
    },
];

#[cfg(all(windows, target_arch = "x86_64"))]
const STOCKFISH: &'static [Asset] = &[
    Asset {
        name: "stockfish-x86-64-bmi2.exe",
        data: include_bytes!("../assets/stockfish-x86-64-bmi2.exe.xz"),
        needs: Cpu::SF_BMI2,
        executable: true,
    },
    Asset {
        name: "stockfish-x86-64-avx2.exe",
        data: include_bytes!("../assets/stockfish-x86-64-avx2.exe.xz"),
        needs: Cpu::SF_AVX2,
        executable: true,
    },
    Asset {
        name: "stockfish-x86-64-sse41-popcnt.exe",
        data: include_bytes!("../assets/stockfish-x86-64-sse41-popcnt.exe.xz"),
        needs: Cpu::SF_SSE41_POPCNT,
        executable: true,
    },
    Asset {
        name: "stockfish-x86-64-ssse3.exe",
        data: include_bytes!("../assets/stockfish-x86-64-ssse3.exe.xz"),
        needs: Cpu::SF_SSSE3,
        executable: true,
    },
    Asset {
        name: "stockfish-x86-64.exe",
        data: include_bytes!("../assets/stockfish-x86-64.exe.xz"),
        needs: Cpu::SF,
        executable: true,
    },
];

#[cfg(all(windows, target_arch = "x86_64"))]
const STOCKFISH_MV: &'static [Asset] = &[
    Asset {
        name: "stockfish-mv-x86-64-bmi2.exe",
        data: include_bytes!("../assets/stockfish-mv-x86-64-bmi2.exe.xz"),
        needs: Cpu::SF_BMI2,
        executable: true,
    },
    Asset {
        name: "stockfish-mv-x86-64-avx2.exe",
        data: include_bytes!("../assets/stockfish-mv-x86-64-avx2.exe.xz"),
        needs: Cpu::SF_AVX2,
        executable: true,
    },
    Asset {
        name: "stockfish-mv-x86-64-sse41-popcnt.exe",
        data: include_bytes!("../assets/stockfish-mv-x86-64-sse41-popcnt.exe.xz"),
        needs: Cpu::SF_SSE41_POPCNT,
        executable: true,
    },
    Asset {
        name: "stockfish-mv-x86-64-ssse3.exe",
        data: include_bytes!("../assets/stockfish-mv-x86-64-ssse3.exe.xz"),
        needs: Cpu::SF_SSSE3,
        executable: true,
    },
    Asset {
        name: "stockfish-mv-x86-64.exe",
        data: include_bytes!("../assets/stockfish-mv-x86-64.exe.xz"),
        needs: Cpu::SF,
        executable: true,
    },
];

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const STOCKFISH: &'static [Asset] = &[
    Asset {
        name: "stockfish-macos-x86-64",
        data: include_bytes!("../assets/stockfish-macos-x86-64.xz"),
        needs: Cpu::SF,
        executable: true,
    },
];

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const STOCKFISH_MV: &'static [Asset] = &[
    Asset {
        name: "stockfish-mv-macos-x86-64",
        data: include_bytes!("../assets/stockfish-mv-macos-x86-64.xz"),
        needs: Cpu::SF,
        executable: true,
    },
];

#[cfg(all(unix, target_arch = "aarch64"))]
const STOCKFISH: &'static [Asset] = &[
    Asset {
        name: "stockfish-mv-armv8",
        data: include_bytes!("../assets/stockfish-armv8.xz"),
        needs: Cpu::SF,
        executable: true,
    },
];

#[cfg(all(unix, target_arch = "aarch64"))]
const STOCKFISH_MV: &'static [Asset] = &[
    Asset {
        name: "stockfish-mv-armv8",
        data: include_bytes!("../assets/stockfish-mv-armv8.xz"),
        needs: Cpu::SF,
        executable: true,
    },
];

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EngineFlavor {
    Official,
    MultiVariant,
}

#[derive(Debug)]
pub struct ByEngineFlavor<T> {
    pub official: T,
    pub multi_variant: T,
}

impl<T> ByEngineFlavor<T> {
    pub fn get(&self, flavor: EngineFlavor) -> &T {
        match flavor {
            EngineFlavor::Official => &self.official,
            EngineFlavor::MultiVariant => &self.multi_variant,
        }
    }

    pub fn get_mut(&mut self, flavor: EngineFlavor) -> &mut T {
        match flavor {
            EngineFlavor::Official => &mut self.official,
            EngineFlavor::MultiVariant => &mut self.multi_variant,
        }
    }
}

#[derive(Debug)]
pub struct Assets {
    dir: TempDir,
    pub nnue: String,
    pub stockfish: ByEngineFlavor<PathBuf>,
}

impl Assets {
    pub fn prepare(cpu: Cpu) -> io::Result<Assets> {
        let dir = tempfile::Builder::new().prefix("fishnet-").tempdir()?;
        Ok(Assets {
            nnue: NNUE.create(dir.path())?.to_str().expect("nnue path printable").to_owned(),
            stockfish: ByEngineFlavor {
                official: STOCKFISH.iter().filter(|a| cpu.contains(a.needs)).next().expect("stockfish").create(dir.path())?,
                multi_variant: STOCKFISH_MV.iter().filter(|a| cpu.contains(a.needs)).next().expect("stockfish").create(dir.path())?,
            },
            dir,
        })
    }
}
