use std::fs::File;
use std::io::{BufWriter, Result as IoResult, Write};

pub const MAX_LINE_COUNT: u32 = u32::MAX;
pub const MAX_COLUMN_COUNT: u32 = u32::MAX;   

// data is check box by line and whether it is toggled.
fn store_checkbox_state(data: &[(u32, bool)], path: &str) -> IoResult<()> {
    let file = File::create(path)?;
    let mut buf = BufWriter::new(file);
    for (line, cond) in data {
        buf.write_all(&line.to_le_bytes())?;
        let bool_byte = if *cond { 1u8 } else { 0u8 };
        buf.write_all(&[bool_byte])?;
    }
    buf.flush()?; // ensure everything is pushed to disk
    Ok(())
}

pub fn main() {}
