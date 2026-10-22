use std::io::{self, Read, Write};
use std::process::ExitCode;
use std::{ptr, thread, time::Duration};

use windows_sys::Win32::Foundation::{GlobalFree, HANDLE};
use windows_sys::Win32::Globalization::{MultiByteToWideChar, CP_ACP};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;

const USAGE: &str = "\
ucopy - UTF-8 클립보드 도구

사용법:
  <명령> | ucopy        stdin(UTF-8)을 클립보드에 복사
  ucopy -p, --paste     클립보드 내용을 UTF-8 로 stdout 에 출력
  -n                    복사/붙여넣기 시 앞뒤 개행 제거 (예: ucopy -n, ucopy -p -n)
  ucopy -h, --help      도움말
";

/// 클립보드를 열고 스코프가 끝나면 닫는다. 다른 프로세스가 점유 중일 수 있어 재시도한다.
struct Clipboard;

impl Clipboard {
    fn open() -> Result<Self, String> {
        for _ in 0..10 {
            if unsafe { OpenClipboard(ptr::null_mut()) } != 0 {
                return Ok(Clipboard);
            }
            thread::sleep(Duration::from_millis(20));
        }
        Err("클립보드를 열 수 없습니다".into())
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe { CloseClipboard() };
    }
}

/// 입력 바이트를 UTF-16 으로 변환한다.
/// UTF-16LE BOM → UTF-16, 유효한 UTF-8 → UTF-8(BOM 제거), 그 외 → 시스템 ANSI 코드페이지.
fn decode(bytes: &[u8]) -> Vec<u16> {
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return rest
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.encode_utf16().collect(),
        Err(_) => ansi_to_utf16(bytes),
    }
}

fn ansi_to_utf16(bytes: &[u8]) -> Vec<u16> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let len = bytes.len() as i32;
    unsafe {
        let n = MultiByteToWideChar(CP_ACP, 0, bytes.as_ptr(), len, ptr::null_mut(), 0);
        let mut out = vec![0u16; n.max(0) as usize];
        MultiByteToWideChar(CP_ACP, 0, bytes.as_ptr(), len, out.as_mut_ptr(), n);
        out
    }
}

/// 앞뒤의 개행(CR/LF)을 제거한다.
fn trim_newlines(wide: &mut Vec<u16>) {
    let is_nl = |c: &u16| *c == 0x0A || *c == 0x0D;
    while wide.last().is_some_and(is_nl) {
        wide.pop();
    }
    let start = wide.iter().take_while(|c| is_nl(c)).count();
    wide.drain(..start);
}

fn copy(trim: bool) -> Result<(), String> {
    let mut input = Vec::new();
    io::stdin()
        .lock()
        .read_to_end(&mut input)
        .map_err(|e| format!("stdin 읽기 실패: {e}"))?;

    let mut wide = decode(&input);
    if trim {
        trim_newlines(&mut wide);
    }
    wide.push(0);
    let size = wide.len() * 2;

    let _cb = Clipboard::open()?;
    unsafe {
        let hmem = GlobalAlloc(GMEM_MOVEABLE, size);
        if hmem.is_null() {
            return Err("메모리 할당 실패".into());
        }
        let dst = GlobalLock(hmem) as *mut u16;
        if dst.is_null() {
            GlobalFree(hmem);
            return Err("메모리 잠금 실패".into());
        }
        ptr::copy_nonoverlapping(wide.as_ptr(), dst, wide.len());
        GlobalUnlock(hmem);

        EmptyClipboard();
        // 성공하면 클립보드가 hmem 소유권을 가져간다
        if SetClipboardData(CF_UNICODETEXT as u32, hmem as HANDLE).is_null() {
            GlobalFree(hmem);
            return Err("클립보드 설정 실패".into());
        }
    }
    Ok(())
}

fn paste(trim: bool) -> Result<(), String> {
    let text = {
        let _cb = Clipboard::open()?;
        unsafe {
            let h = GetClipboardData(CF_UNICODETEXT as u32);
            if h.is_null() {
                return Ok(()); // 텍스트가 없으면 빈 출력
            }
            let p = GlobalLock(h) as *const u16;
            if p.is_null() {
                return Err("메모리 잠금 실패".into());
            }
            let mut len = 0;
            while *p.add(len) != 0 {
                len += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
            GlobalUnlock(h);
            s
        }
    };
    let text = if trim {
        text.trim_matches(|c| c == '\n' || c == '\r')
    } else {
        &text
    };
    let mut out = io::stdout().lock();
    out.write_all(text.as_bytes())
        .and_then(|_| out.flush())
        .map_err(|e| format!("stdout 쓰기 실패: {e}"))
}

fn main() -> ExitCode {
    let mut paste_mode = false;
    let mut trim = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-p" | "--paste" => paste_mode = true,
            "-n" => trim = true,
            "-h" | "--help" => {
                print!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("알 수 없는 옵션: {other}\n\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let result = if paste_mode { paste(trim) } else { copy(trim) };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ucopy: {e}");
            ExitCode::FAILURE
        }
    }
}
