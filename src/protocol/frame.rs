// The first two bytes identify messages from our protocol.
pub const MAGIC: [u8; 2] = *b"LO";

// The decoder will reject versions it does not understand.
pub const WIRE_VERSION: u8 = 1;

// Header: magic (2) + version (1) + type (1) + length (2) + flags (2).
pub const HEADER_LEN: usize = 8;

// Header (8) + instrument UUID (16) + three u64 fields (24)
// + side (1) + reserved bytes (7).
pub const FRAME_LEN: usize = 56;

/// The three operations a client can send in version 1.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    Add = 1,
    Cancel = 2,
    Modify = 3,
}
