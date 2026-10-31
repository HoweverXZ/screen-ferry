//! 协议上限。编码和解码都会检查，超出即视为错误，保证内存占用有界。

/// 设备名的最大字节数（UTF-8）。
pub const MAX_NAME_LEN: usize = 128;

/// 一台设备最多声明的显示器数量。
pub const MAX_DISPLAYS: usize = 32;

/// 一次剪贴板通告中最多的格式数量。
pub const MAX_CLIPBOARD_FORMATS: usize = 16;

/// 单个数据分块的最大字节数。
pub const MAX_CHUNK_LEN: usize = 64 * 1024;

/// 输入连接上单帧消息体的最大字节数。
pub const MAX_INPUT_PAYLOAD: usize = 4 * 1024;

/// 数据连接上单帧消息体的最大字节数：一个满载分块再加少量字段。
pub const MAX_DATA_PAYLOAD: usize = MAX_CHUNK_LEN + 64;
