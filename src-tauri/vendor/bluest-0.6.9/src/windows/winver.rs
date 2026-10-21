use std::mem;

use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::SystemInformation::OSVERSIONINFOW;
use windows::{s, w};

fn get_windows_version() -> Option<(u32, u32, u32)> {
    let handle = unsafe { GetModuleHandleW(w!("ntdll.dll")).ok()? };
    let proc = unsafe { GetProcAddress(handle, s!("RtlGetVersion"))? };

    type RtlGetVersionFunc = unsafe extern "system" fn(*mut OSVERSIONINFOW) -> i32;
    let proc: RtlGetVersionFunc = unsafe { mem::transmute(proc) };

    let mut info: OSVERSIONINFOW = unsafe { mem::zeroed() };
    info.dwOSVersionInfoSize = mem::size_of::<OSVERSIONINFOW>() as u32;

    let status = unsafe { proc(&mut info) };
    if status != 0 {
        None
    } else {
        Some((info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber))
    }
}

pub fn windows_version_above(major: u32, minor: u32, build: u32) -> bool {
    let Some((cur_major, cur_minor, cur_build)) = get_windows_version() else {
        return false;
    };
    cur_major
        .cmp(&major)
        .then_with(|| cur_minor.cmp(&minor))
        .then_with(|| cur_build.cmp(&build))
        .is_ge()
}
