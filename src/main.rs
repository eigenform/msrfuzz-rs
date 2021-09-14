//! Goofy way of enumerating "acceptable" MSRs via /dev/cpu/n/msr, where the
//! word "acceptable" here means "cases where RDMSR doesn't fault."

use std::collections::BTreeMap;

// I ran through the entire 32-bit space of ECX values on my 3950X, and 
// there was nothing outside the expected ranges of architectural MSRs.
// These ranges should gather all of the acceptable values.

const REGION_LO_3950X: std::ops::Range<u32> = 0x0000_0000..0x0000_1000;
const REGION_HI_3950X: std::ops::Range<u32> = 0xc000_0000..0xc002_0000;

/// Open the MSR device.
pub fn msr_open(core_id: usize) -> Result<i32, &'static str> {
    let path = format!("/dev/cpu/{}/msr", core_id);
    match nix::fcntl::open(path.as_str(), nix::fcntl::OFlag::O_RDONLY, 
                           nix::sys::stat::Mode::S_IRUSR) {
        Ok(fd) => Ok(fd),
        Err(e) => match e {
            nix::Error::Sys(eno) => match eno {
                nix::errno::Errno::EACCES => Err("Permission denied"),
                _ => panic!("{}", eno),
            },
            _ => panic!("{}", e),
        },
    }
}

/// Close the MSR device.
pub fn msr_close(fd: i32) {
    use nix::unistd::close;
    match close(fd) {
        Ok(_) => {},
        Err(e) => panic!("{}", e),
    }
}

/// Test an MSR.
pub fn msr_read(fd: i32, msr: u32) -> Result<u64, &'static str> {
    let mut buf = [0u8; 8];
    match nix::sys::uio::pread(fd, &mut buf, msr as i64) {
        Ok(_) => Ok(u64::from_le_bytes(buf)),
        Err(e) => match e {
            nix::Error::Sys(eno) => match eno {
                nix::errno::Errno::EIO => Err("Unsupported MSR"),
                _ => panic!("{}", eno),
            },
            _ => panic!("{}", e),
        },
    }
}

fn main() -> Result<(), &'static str> {

    const TGT_CORE: usize = 0;
    let mut output = BTreeMap::new();

    // Pin to the same core we're reading from.
    // You get a ~10x slowdown when you're not doing this (lol).

    let this_pid = nix::unistd::Pid::from_raw(0);
    let mut cpuset = nix::sched::CpuSet::new();
    cpuset.set(TGT_CORE).unwrap();
    nix::sched::sched_setaffinity(this_pid, &cpuset).unwrap();

    let fd = match msr_open(TGT_CORE) {
        Ok(fd) => fd,
        Err(e) => return Err(e),
    };

    for msr in REGION_LO_3950X {
        if let Ok(val) = msr_read(fd, msr) {
            eprintln!("Found MSR {:08x}", msr);
            output.insert(msr, val);
        }
    }
    for msr in REGION_HI_3950X {
        if let Ok(val) = msr_read(fd, msr) {
            eprintln!("Found MSR {:08x}", msr);
            output.insert(msr, val);
        }
    }

    for (msr, val) in &output {
        println!("{:08x}: {:016x}", msr, val);
    }

    msr_close(fd);
    Ok(())
}
