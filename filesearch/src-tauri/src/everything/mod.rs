use std::ffi::{CString, CStr};
use std::os::raw::{c_char, c_void, c_int, c_uint};
use std::ptr;

/// Everything SDK constants
const EVERYTHING_OK: i32 = 0;
const EVERYTHING_ERROR_MEMORY: i32 = 1;
const EVERYTHING_ERROR_IPC: i32 = 2;
const EVERYTHING_ERROR_REGISTERCLASS: i32 = 3;
const EVERYTHING_ERROR_CREATEWINDOW: i32 = 4;
const EVERYTHING_ERROR_CREATETHREAD: i32 = 5;
const EVERYTHING_ERROR_INVALIDINDEX: i32 = 6;
const EVERYTHING_ERROR_INVALIDCALL: i32 = 7;

const EVERYTHING_REQUEST_FILE_NAME: u32 = 0x00000001;
const EVERYTHING_REQUEST_PATH: u32 = 0x00000002;
const EVERYTHING_REQUEST_FULL_PATH_AND_FILE_NAME: u32 = 0x00000004;
const EVERYTHING_REQUEST_SIZE: u32 = 0x00000010;
const EVERYTHING_REQUEST_DATE_MODIFIED: u32 = 0x00000040;
const EVERYTHING_REQUEST_DATE_CREATED: u32 = 0x00000080;
const EVERYTHING_REQUEST_ATTRIBUTES: u32 = 0x00000100;
const EVERYTHING_REQUEST_EXTENSION: u32 = 0x00000800;

/// Everything SDK result
#[derive(Debug, Clone, serde::Serialize)]
pub struct EverythingResult {
    pub name: String,
    pub path: String,
    pub full_path: String,
    pub size: u64,
    pub modified: Option<u64>,
    pub created: Option<u64>,
    pub is_folder: bool,
    pub extension: Option<String>,
}

/// Everything SDK wrapper
pub struct EverythingSDK {
    loaded: bool,
    // Function pointers would be here for dynamic loading
}

impl EverythingSDK {
    pub fn new() -> Self {
        Self { loaded: false }
    }

    /// Check if Everything is running
    #[cfg(windows)]
    pub fn is_everything_running(&self) -> bool {
        // Try to find Everything window
        use winapi::um::winuser::FindWindowA;

        unsafe {
            let class_name = CString::new("EVERYTHING").unwrap();
            let hwnd = FindWindowA(class_name.as_ptr(), ptr::null());

            !hwnd.is_null()
        }
    }

    #[cfg(not(windows))]
    pub fn is_everything_running(&self) -> bool {
        false
    }

    /// Search using Everything
    #[cfg(windows)]
    pub fn search(&mut self, query: &str, max_results: u32) -> Result<Vec<EverythingResult>, String> {
        if !self.is_everything_running() {
            return Err("Everything is not running. Please start Everything first.".to_string());
        }

        // For now, use IPC via named pipe
        self.search_via_ipc(query, max_results)
    }

    #[cfg(not(windows))]
    pub fn search(&mut self, _query: &str, _max_results: u32) -> Result<Vec<EverythingResult>, String> {
        Err("Everything SDK is only available on Windows".to_string())
    }

    /// Search via IPC (inter-process communication)
    #[cfg(windows)]
    fn search_via_ipc(&self, query: &str, max_results: u32) -> Result<Vec<EverythingResult>, String> {
        use std::io::{Read, Write};
        use std::os::windows::io::{FromRawHandle, IntoRawHandle};

        // Everything IPC query format
        // Query: "query_string"
        // Returns: one result per line

        let query_str = format!(
            "GET\r\n{}\r\ncount={}\r\npath_column=1\r\nsize_column=1\r\ndate_modified_column=1\r\n\r\n",
            query, max_results
        );

        // Named pipe: \\.\pipe\EVERYTHING_IPC
        let pipe_name = r"\\.\pipe\EVERYTHING_IPC";

        let mut results = Vec::new();

        // Open pipe
        unsafe {
            use winapi::um::fileapi::CreateFileA;
            use winapi::um::namedpipeapi::WaitNamedPipeA;
            use winapi::um::winbase::{PIPE_ACCESS_DUPLEX, WAIT_OBJECT_0};
            use winapi::um::winnt::{GENERIC_READ, GENERIC_WRITE};
            use winapi::um::synchapi::WaitForSingleObject;
            use winapi::um::handleapi::INVALID_HANDLE_VALUE;

            let pipe_name_c = CString::new(pipe_name).unwrap();

            // Wait for pipe
            WaitNamedPipeA(pipe_name_c.as_ptr(), 5000);

            // Open pipe
            let handle = CreateFileA(
                pipe_name_c.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                ptr::null_mut(),
                winapi::um::fileapi::OPEN_EXISTING,
                0,
                ptr::null_mut(),
            );

            if handle == INVALID_HANDLE_VALUE {
                return Err("Cannot connect to Everything IPC".to_string());
            }

            // Write query
            use winapi::um::fileapi::WriteFile;
            let query_bytes = query_str.as_bytes();

            let mut bytes_written: u32 = 0;
            let result = WriteFile(
                handle,
                query_bytes.as_ptr() as *const c_void,
                query_bytes.len() as u32,
                &mut bytes_written,
                ptr::null_mut(),
            );

            if result == 0 {
                winapi::um::handleapi::CloseHandle(handle);
                return Err("Failed to write query to Everything".to_string());
            }

            // Read response
            use winapi::um::fileapi::ReadFile;
            let mut buffer = [0u8; 65536];
            let mut total_read = 0;
            let mut response = Vec::new();

            loop {
                let mut bytes_read: u32 = 0;
                let result = ReadFile(
                    handle,
                    buffer.as_mut_ptr() as *mut c_void,
                    buffer.len() as u32,
                    &mut bytes_read,
                    ptr::null_mut(),
                );

                if result == 0 || bytes_read == 0 {
                    break;
                }

                response.extend_from_slice(&buffer[..bytes_read as usize]);
                total_read += bytes_read;

                if bytes_read < buffer.len() as u32 {
                    break;
                }
            }

            winapi::um::handleapi::CloseHandle(handle);

            // Parse response
            let response_str = String::from_utf8_lossy(&response);
            let lines: Vec<&str> = response_str.lines().collect();

            for line in lines {
                if line.is_empty() || line.starts_with('.') {
                    continue;
                }

                // Parse line: path\to\file
                let path = line.to_string();
                let name = std::path::Path::new(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                let ext = std::path::Path::new(&path)
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase());

                let is_folder = std::path::Path::new(&path).is_dir();

                results.push(EverythingResult {
                    name,
                    path: std::path::Path::new(&path)
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    full_path: path,
                    size: 0, // Would need additional queries
                    modified: None,
                    created: None,
                    is_folder: is_folder,
                    extension: ext,
                });

                if results.len() >= max_results as usize {
                    break;
                }
            }
        }

        Ok(results)
    }

    /// Get Everything version
    #[cfg(windows)]
    pub fn get_version(&self) -> Result<String, String> {
        if !self.is_everything_running() {
            return Err("Everything is not running".to_string());
        }

        // Would query Everything version via IPC
        Ok("Everything 1.4.x".to_string())
    }

    #[cfg(not(windows))]
    pub fn get_version(&self) -> Result<String, String> {
        Err("Everything SDK is only available on Windows".to_string())
    }
}

impl Default for EverythingSDK {
    fn default() -> Self {
        Self::new()
    }
}

/// Alternative: Use everything.exe command line
pub struct EverythingCLI {
    exe_path: Option<String>,
}

impl EverythingCLI {
    pub fn new() -> Self {
        // Try to find es.exe (Everything command line)
        let exe_path = Self::find_es_exe();
        Self { exe_path }
    }

    /// Find es.exe in common locations
    fn find_es_exe() -> Option<String> {
        let common_paths = [
            r"C:\Program Files\Everything\es.exe",
            r"C:\Program Files (x86)\Everything\es.exe",
            r"C:\Everything\es.exe",
        ];

        for path in &common_paths {
            if std::path::Path::new(path).exists() {
                return Some(path.to_string());
            }
        }

        None
    }

    /// Search using es.exe command line
    pub fn search(&self, query: &str, max_results: u32) -> Result<Vec<EverythingResult>, String> {
        let exe = self.exe_path.as_ref()
            .ok_or_else(|| "es.exe not found. Please install Everything and ensure es.exe is in PATH".to_string())?;

        use std::process::Command;

        let output = Command::new(exe)
            .args([
                "-n", &max_results.to_string(),
                "-path",
                query
            ])
            .output()
            .map_err(|e| format!("Failed to execute es.exe: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        for line in stdout.lines() {
            let path = line.to_string();
            let name = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            let ext = std::path::Path::new(&path)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase());

            results.push(EverythingResult {
                name,
                path: std::path::Path::new(&path)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
                full_path: path,
                size: 0,
                modified: None,
                created: None,
                is_folder: false,
                extension: ext,
            });
        }

        Ok(results)
    }
}

impl Default for EverythingCLI {
    fn default() -> Self {
        Self::new()
    }
}