use crate::ClientId;

#[derive(Debug, PartialEq)]
#[repr(u32)]
pub enum ClientMessage {
    ClientConnected(ClientId) = 0u32,
    ClientDisconnected(ClientId) = 1u32,
    Text = 2u32,
    Image = 3u32,
}

fn read_u32(bytes: &[u8]) -> Result<u32, std::array::TryFromSliceError> {
    let slice = &bytes[..4];
    let arr: [u8; 4] = slice.try_into()?;
    Ok(u32::from_be_bytes(arr))
}
