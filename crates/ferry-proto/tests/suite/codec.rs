use ferry_proto::{
    Capabilities, Channel, Codec, DecodeError, EncodeError, HEADER_LEN, Hello, Message,
};
use ferry_types::{InputEvent, KeyAction, KeyCode};

use crate::support::{Rng, encode, frame, samples};

#[test]
fn waits_until_a_whole_frame_arrives() {
    for (channel, message) in samples() {
        let buf = encode(channel, &message);
        for cut in 0..buf.len() {
            assert_eq!(
                Codec::new(channel).decode(&buf[..cut]),
                Ok(None),
                "{message:?} 截到 {cut}"
            );
        }
    }
}

#[test]
fn decodes_back_to_back_frames() {
    let codec = Codec::new(Channel::Input);
    let sent: Vec<Message> = samples()
        .into_iter()
        .filter(|(channel, _)| *channel == Channel::Input)
        .map(|(_, m)| m)
        .collect();

    let mut stream = Vec::new();
    for message in &sent {
        codec.encode(message, &mut stream).unwrap();
    }

    let mut received = Vec::new();
    let mut rest = &stream[..];
    while let Some((message, used)) = codec.decode(rest).unwrap() {
        received.push(message);
        rest = &rest[used..];
    }
    assert!(rest.is_empty());
    assert_eq!(received, sent);
}

#[test]
fn rejects_bad_headers_before_the_body_arrives() {
    let header_only = |ty: u16, len: u32| {
        let mut buf = ty.to_le_bytes().to_vec();
        buf.extend_from_slice(&len.to_le_bytes());
        buf
    };
    let input = Codec::new(Channel::Input);

    assert_eq!(
        input.decode(&header_only(0x7777, 0)),
        Err(DecodeError::UnknownMessage(0x7777))
    );
    assert_eq!(
        input.decode(&header_only(0x0005, u32::MAX)),
        Err(DecodeError::FrameTooLarge {
            len: u32::MAX as usize,
            max: input.max_payload()
        }),
    );
    assert_eq!(
        Codec::new(Channel::Data).decode(&header_only(0x0110, 5)),
        Err(DecodeError::WrongChannel {
            ty: 0x0110,
            channel: Channel::Data
        }),
    );
}

#[test]
fn input_frames_are_capped_tighter_than_data_frames() {
    let input = Codec::new(Channel::Input);
    let data = Codec::new(Channel::Data);
    assert!(input.max_payload() < data.max_payload());

    let oversized = frame(0x0005, &vec![0; input.max_payload() + 1]);
    assert!(matches!(
        input.decode(&oversized),
        Err(DecodeError::FrameTooLarge { .. })
    ));
}

#[test]
fn refuses_to_send_on_the_wrong_channel() {
    let key = Message::Input(InputEvent::Key {
        code: KeyCode::LEFT_CTRL,
        action: KeyAction::Up,
    });
    let mut buf = vec![1, 2, 3];
    assert_eq!(
        Codec::new(Channel::Data).encode(&key, &mut buf),
        Err(EncodeError::WrongChannel(Channel::Data)),
    );
    assert_eq!(buf, [1, 2, 3]);
}

#[test]
fn failed_encode_leaves_the_buffer_untouched() {
    let long_name = Hello::new(Channel::Input, Capabilities::empty(), 0, "x".repeat(1000));
    let mut buf = vec![1, 2, 3];
    assert!(
        Codec::new(Channel::Input)
            .encode(&Message::Hello(long_name), &mut buf)
            .is_err()
    );
    assert_eq!(buf, [1, 2, 3]);
}

/// 随机破坏合法帧：解码不能 panic；凡是能解出来的消息，重新编码必须得到完全相同的字节。
#[test]
fn corrupted_frames_never_panic_and_decoding_is_canonical() {
    let mut rng = Rng::new(0x5EED);
    for (channel, message) in samples() {
        let codec = Codec::new(channel);
        let original = encode(channel, &message);

        for _ in 0..2_000 {
            let mut buf = original.clone();
            for _ in 0..=rng.below(3) {
                let i = rng.below(buf.len());
                buf[i] ^= 1 << rng.below(8);
            }

            if let Ok(Some((decoded, used))) = codec.decode(&buf) {
                let mut again = Vec::new();
                codec
                    .encode(&decoded, &mut again)
                    .expect("解出的消息必须能重新编码");
                assert_eq!(again, buf[..used]);
            }
        }
    }
}

#[test]
fn random_bytes_never_panic() {
    let mut rng = Rng::new(42);
    for channel in [Channel::Input, Channel::Data, Channel::Pair] {
        let codec = Codec::new(channel);
        for _ in 0..20_000 {
            let len = rng.below(64);
            let mut buf: Vec<u8> = (0..len).map(|_| rng.next_u64() as u8).collect();
            // 一半的样本给一个合法的类型号，让解码走得更深。
            if buf.len() >= HEADER_LEN && rng.below(2) == 0 {
                let ty: u16 =
                    [0x0001, 0x0005, 0x0008, 0x0101, 0x0122, 0x0200, 0x0300][rng.below(7)];
                buf[..2].copy_from_slice(&ty.to_le_bytes());
                let body = (buf.len() - HEADER_LEN) as u32;
                buf[2..HEADER_LEN].copy_from_slice(&body.to_le_bytes());
            }
            let _ = codec.decode(&buf);
        }
    }
}

#[test]
fn pairing_messages_stay_on_the_pairing_channel() {
    let nonce = Message::PairNonce { nonce: [1; 32] };
    let mut buf = Vec::new();
    for channel in [Channel::Input, Channel::Data] {
        assert_eq!(
            Codec::new(channel).encode(&nonce, &mut buf),
            Err(EncodeError::WrongChannel(channel))
        );
    }
    let frame = encode(Channel::Pair, &nonce);
    assert_eq!(frame.len(), HEADER_LEN + 32, "随机数定长，不带长度前缀");
    assert!(matches!(
        Codec::new(Channel::Input).decode(&frame),
        Err(DecodeError::WrongChannel { ty: 0x0008, .. })
    ));
}
