//! 任意字节交给解码器：不能 panic；解出的每条消息重新编码后，必须与原字节完全相同。

#![no_main]

use ferry_proto::{Channel, Codec};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    for channel in [Channel::Input, Channel::Data] {
        let codec = Codec::new(channel);
        let mut rest = data;
        while let Ok(Some((message, used))) = codec.decode(rest) {
            let mut again = Vec::new();
            codec.encode(&message, &mut again).expect("解出的消息必须能重新编码");
            assert_eq!(again, rest[..used]);
            rest = &rest[used..];
        }
    }
});
