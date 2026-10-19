// 密钥只以当前 Windows 用户可解密的 DPAPI 密文落盘。
#[cfg(windows)]
pub fn transform(data: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    use std::{ffi::c_void, ptr};
    #[repr(C)]
    struct Blob { size: u32, data: *mut u8 }
    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(input: *const Blob, description: *const u16, entropy: *const Blob, reserved: *mut c_void, prompt: *mut c_void, flags: u32, output: *mut Blob) -> i32;
        fn CryptUnprotectData(input: *const Blob, description: *mut *mut u16, entropy: *const Blob, reserved: *mut c_void, prompt: *mut c_void, flags: u32, output: *mut Blob) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" { fn LocalFree(memory: *mut c_void) -> *mut c_void; }
    let input = Blob { size: data.len().try_into().map_err(|_| "密钥数据过长")?, data: data.as_ptr() as *mut u8 };
    let mut output = Blob { size: 0, data: ptr::null_mut() };
    unsafe {
        let result = if encrypt {
            CryptProtectData(&input, ptr::null(), ptr::null(), ptr::null_mut(), ptr::null_mut(), 1, &mut output)
        } else {
            CryptUnprotectData(&input, ptr::null_mut(), ptr::null(), ptr::null_mut(), ptr::null_mut(), 1, &mut output)
        };
        if result == 0 { return Err("无法保护或解密 API Key，请使用当前 Windows 用户重新配置密钥".into()); }
        let bytes = std::slice::from_raw_parts(output.data, output.size as usize).to_vec();
        LocalFree(output.data as *mut c_void);
        Ok(bytes)
    }
}

#[cfg(not(windows))]
pub fn transform(_: &[u8], _: bool) -> Result<Vec<u8>, String> { Err("当前版本的 API Key 加密保存需要 Windows".into()) }
