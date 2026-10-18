//! What the page's script calls. Bytes cross through two buffers this module owns: the script
//! asks for an input buffer of a size, writes into WebAssembly memory at the address it gets,
//! then calls `mks_open` or `mks_call`, which leave the dashboard's HTML in the output buffer
//! and return its address (`mks_output_len` its length). Addresses are only handed out, never
//! read from: there is no unsafe code here, only the attribute that exports a name.

use std::cell::RefCell;

use crate::Board;

thread_local! {
    static BOARD: RefCell<Option<Board>> = const { RefCell::new(None) };
    static INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// The output buffer holds `html`; returns its address.
fn answer(html: String) -> u32 {
    OUTPUT.with(|o| {
        *o.borrow_mut() = html.into_bytes();
        o.borrow().as_ptr() as usize as u32
    })
}

/// A zeroed input buffer of `len` bytes; returns its address for the script to write to.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn mks_input(len: u32) -> u32 {
    INPUT.with(|i| {
        *i.borrow_mut() = vec![0; len as usize];
        i.borrow().as_ptr() as usize as u32
    })
}

/// Opens the dashboard from the payload in the input buffer.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn mks_open() -> u32 {
    let payload = INPUT.with(|i| std::mem::take(&mut *i.borrow_mut()));
    let html = match Board::open(&payload) {
        Ok(board) => {
            let html = board.render();
            BOARD.with(|b| *b.borrow_mut() = Some(board));
            html
        }
        Err(e) => format!("<p class=\"warning\">{}</p>", mksigma_engine::output::html::escape(&e)),
    };
    answer(html)
}

/// Runs the command in the input buffer (see `Board::call`).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn mks_call() -> u32 {
    let command = INPUT.with(|i| String::from_utf8_lossy(&std::mem::take(&mut *i.borrow_mut())).into_owned());
    let html = BOARD.with(|b| match b.borrow_mut().as_mut() {
        Some(board) => board.call(&command),
        None => "<p class=\"warning\">The dashboard didn't open.</p>".to_string(),
    });
    answer(html)
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn mks_output_len() -> u32 {
    OUTPUT.with(|o| o.borrow().len() as u32)
}
