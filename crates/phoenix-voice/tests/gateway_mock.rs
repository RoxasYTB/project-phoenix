use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use phoenix_voice::gateway::{self, DaveServerMessage, GatewayEvent, ReconnectPolicy, ServerMessage, VoiceConnectInfo};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;

fn op_of(m: &Message) -> Option<(u64, Value)> {
    let Message::Text(t) = m else { return None };
    let v: Value = serde_json::from_str(t.as_str()).ok()?;
    Some((v["op"].as_u64()?, v["d"].clone()))
}

async fn next_event(rx: &mut tokio::sync::mpsc::UnboundedReceiver<GatewayEvent>) -> GatewayEvent {
    tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.expect("event timeout").expect("channel")
}

#[tokio::test]
async fn identify_heartbeat_dave_binary_resume() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (s, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(s).await.unwrap();
        ws.send(Message::Text(r#"{"op":8,"d":{"heartbeat_interval":100}}"#.into())).await.unwrap();
        let (op, d) = op_of(&ws.next().await.unwrap().unwrap()).unwrap();
        assert_eq!(op, 0);
        assert_eq!(d["session_id"], "sess");
        assert_eq!(d["max_dave_protocol_version"], 1);
        ws.send(Message::Text(r#"{"op":2,"seq":1,"d":{"ssrc":11,"ip":"127.0.0.1","port":4000,"modes":["aead_aes256_gcm_rtpsize"],"streams":[]}}"#.into())).await.unwrap();
        ws.send(Message::Binary(vec![0, 2, 25, 0xde, 0xad].into())).await.unwrap();
        let mut got_hb = false;
        let mut got_kp = false;
        let mut got_video = false;
        while !(got_hb && got_kp && got_video) {
            let m = ws.next().await.unwrap().unwrap();
            match &m {
                Message::Binary(b) => {
                    assert_eq!(b[0], 26);
                    assert_eq!(&b[1..], &[1, 2, 3]);
                    got_kp = true;
                }
                _ => {
                    let (op, d) = op_of(&m).unwrap();
                    match op {
                        3 if d["seq_ack"] == 2 => {
                            got_hb = true;
                            ws.send(Message::Text(format!(r#"{{"op":6,"d":{{"t":{}}}}}"#, d["t"]).into())).await.unwrap();
                        }
                        3 => {}
                        12 => {
                            assert_eq!(d["streams"][0]["ssrc"], 12);
                            got_video = true;
                        }
                        other => panic!("unexpected op {other}"),
                    }
                }
            }
        }
        ws.close(Some(CloseFrame { code: CloseCode::Library(4015), reason: "server crashed".into() })).await.unwrap();
        drop(ws);

        let (s, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(s).await.unwrap();
        ws.send(Message::Text(r#"{"op":8,"d":{"heartbeat_interval":100}}"#.into())).await.unwrap();
        let (op, d) = op_of(&ws.next().await.unwrap().unwrap()).unwrap();
        assert_eq!(op, 7);
        assert_eq!(d["seq_ack"], 2);
        assert_eq!(d["token"], "tok");
        ws.send(Message::Text(r#"{"op":9,"d":null}"#.into())).await.unwrap();
        while let Some(Ok(m)) = ws.next().await {
            if let Message::Close(_) = m {
                break;
            }
        }
    });

    let info = VoiceConnectInfo {
        endpoint: "unused".into(),
        server_id: "1".into(),
        user_id: "2".into(),
        session_id: "sess".into(),
        token: "tok".into(),
        max_dave_protocol_version: 1,
        stream: false,
    };
    let (h, mut rx, task) = gateway::spawn(
        info,
        Some(format!("ws://{addr}")),
        ReconnectPolicy { max_attempts: 2, backoff: Duration::from_millis(10), ..ReconnectPolicy::default() },
    );

    assert!(matches!(next_event(&mut rx).await, GatewayEvent::Message(ServerMessage::Hello { .. })));
    let GatewayEvent::Message(ServerMessage::Ready(r)) = next_event(&mut rx).await else { panic!("ready") };
    assert_eq!(r.ssrc, 11);
    assert_eq!(
        next_event(&mut rx).await,
        GatewayEvent::Message(ServerMessage::Dave(DaveServerMessage::ExternalSender(vec![0xde, 0xad])))
    );
    h.dave_key_package(vec![1, 2, 3]).unwrap();
    h.video(&phoenix_voice::gateway::VideoPayload {
        user_id: None,
        audio_ssrc: 11,
        video_ssrc: 12,
        rtx_ssrc: 13,
        streams: vec![phoenix_voice::gateway::protocol::StreamDescriptor {
            kind: "video".into(),
            rid: "100".into(),
            ssrc: 12,
            rtx_ssrc: 13,
            active: true,
            quality: 100,
            ..Default::default()
        }],
    })
    .unwrap();

    loop {
        match next_event(&mut rx).await {
            GatewayEvent::Reconnecting { code } => {
                assert_eq!(code, Some(4015));
                break;
            }
            GatewayEvent::Message(_) => {}
            e => panic!("unexpected {e:?}"),
        }
    }
    loop {
        match next_event(&mut rx).await {
            GatewayEvent::Message(ServerMessage::Resumed) => break,
            GatewayEvent::Message(_) => {}
            e => panic!("unexpected {e:?}"),
        }
    }
    h.close().unwrap();
    assert!(task.await.unwrap().is_ok());
    server.await.unwrap();
}

#[tokio::test]
async fn non_resumable_close_ends_task() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (s, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(s).await.unwrap();
        let _ = ws.next().await;
        let _ = ws.close(Some(CloseFrame { code: CloseCode::Library(4004), reason: "auth".into() })).await;
    });
    let info = VoiceConnectInfo {
        endpoint: "x".into(),
        server_id: "1".into(),
        user_id: "2".into(),
        session_id: "s".into(),
        token: "bad".into(),
        max_dave_protocol_version: 0,
        stream: false,
    };
    let (_h, mut rx, task) = gateway::spawn(info, Some(format!("ws://{addr}")), ReconnectPolicy::default());
    assert!(matches!(next_event(&mut rx).await, GatewayEvent::Closed { code: Some(4004), .. }));
    assert!(task.await.unwrap().is_err());
}

#[tokio::test]
async fn go_live_stream_ops_against_the_mock_gateway() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (s, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(s).await.unwrap();
        ws.send(Message::Text(r#"{"op":8,"d":{"heartbeat_interval":5000}}"#.into())).await.unwrap();
        let (op, d) = op_of(&ws.next().await.unwrap().unwrap()).unwrap();
        assert_eq!(op, 0);
        assert_eq!(d["streams"][0]["type"], "screen");
        assert_eq!(d["video"], true);
        ws.send(Message::Text(r#"{"op":2,"seq":1,"d":{"ssrc":3091,"ip":"127.0.0.1","port":4000,"modes":["aead_aes256_gcm_rtpsize"],"streams":[{"type":"video","rid":"100","ssrc":3092,"rtx_ssrc":3093,"active":false,"quality":100}]}}"#.into())).await.unwrap();
        ws.send(Message::Text(r#"{"op":12,"seq":2,"d":{"user_id":"42","audio_ssrc":2732,"video_ssrc":2733,"streams":[{"rid":"100","ssrc":2733,"rtx_ssrc":2734,"active":true,"quality":100,"max_resolution":{"width":1920,"height":1080}}]}}"#.into())).await.unwrap();
        loop {
            let (op, d) = op_of(&ws.next().await.unwrap().unwrap()).unwrap();
            if op != 12 {
                continue;
            }
            assert_eq!((d["audio_ssrc"].as_u64(), d["video_ssrc"].as_u64(), d["rtx_ssrc"].as_u64()), (Some(3091), Some(3092), Some(3093)));
            let s = &d["streams"][0];
            assert_eq!((s["type"].as_str(), s["rid"].as_str(), s["active"].as_bool()), (Some("video"), Some("100"), Some(true)));
            assert_eq!((s["ssrc"].as_u64(), s["rtx_ssrc"].as_u64(), s["max_framerate"].as_u64()), (Some(3092), Some(3093), Some(30)));
            assert_eq!(s["max_resolution"]["width"], 1280);
            break;
        }
        let _ = ws.close(None).await;
    });

    let info = VoiceConnectInfo {
        endpoint: "unused".into(),
        server_id: "1".into(),
        user_id: "2".into(),
        session_id: "sess".into(),
        token: "tok".into(),
        max_dave_protocol_version: 1,
        stream: true,
    };
    let (h, mut rx, _task) = gateway::spawn(info, Some(format!("ws://{addr}")), ReconnectPolicy { max_attempts: 0, backoff: Duration::from_millis(10), ..ReconnectPolicy::default() });
    let mut ready = None;
    let mut video = None;
    while ready.is_none() || video.is_none() {
        match next_event(&mut rx).await {
            GatewayEvent::Message(ServerMessage::Ready(r)) => ready = Some(r),
            GatewayEvent::Message(ServerMessage::Video(v)) => video = Some(v),
            GatewayEvent::Message(_) => {}
            e => panic!("unexpected {e:?}"),
        }
    }
    let v = video.unwrap();
    assert_eq!(v.user_id.as_deref(), Some("42"));
    assert_eq!(phoenix_voice::video_rx::layers(&v), vec![phoenix_voice::video_rx::Layer { ssrc: 2733, rtx_ssrc: 2734, quality: 100 }]);
    let r = ready.unwrap();
    let s = &r.streams[0];
    let mut state = phoenix_voice::connection::VideoSendState::new(true, r.ssrc, s.ssrc, s.rtx_ssrc).with_kind(&s.kind);
    h.video(&state.start(1280, 720, 30, 2_500_000).unwrap()).unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn each_successful_resume_restores_the_reconnect_budget() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        for round in 0..4 {
            let (s, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(s).await.unwrap();
            ws.send(Message::Text(r#"{"op":8,"d":{"heartbeat_interval":10000}}"#.into())).await.unwrap();
            let (op, _) = op_of(&ws.next().await.unwrap().unwrap()).unwrap();
            if round == 0 {
                assert_eq!(op, 0);
                ws.send(Message::Text(r#"{"op":2,"seq":1,"d":{"ssrc":11,"ip":"127.0.0.1","port":4000,"modes":["aead_aes256_gcm_rtpsize"],"streams":[]}}"#.into())).await.unwrap();
            } else {
                assert_eq!(op, 7);
                ws.send(Message::Text(r#"{"op":9,"d":null}"#.into())).await.unwrap();
            }
            if round < 3 {
                ws.close(Some(CloseFrame { code: CloseCode::Library(4015), reason: "server crashed".into() })).await.unwrap();
                while let Some(Ok(_)) = ws.next().await {}
            } else {
                while let Some(Ok(m)) = ws.next().await {
                    if let Message::Close(_) = m {
                        break;
                    }
                }
            }
        }
    });
    let info = VoiceConnectInfo {
        endpoint: "unused".into(),
        server_id: "1".into(),
        user_id: "2".into(),
        session_id: "sess".into(),
        token: "tok".into(),
        max_dave_protocol_version: 0,
        stream: false,
    };
    let (h, mut rx, task) = gateway::spawn(
        info,
        Some(format!("ws://{addr}")),
        ReconnectPolicy { max_attempts: 1, backoff: Duration::from_millis(10), ..ReconnectPolicy::default() },
    );
    let mut resumed = 0;
    while resumed < 3 {
        match next_event(&mut rx).await {
            GatewayEvent::Message(ServerMessage::Resumed) => resumed += 1,
            GatewayEvent::Message(_) | GatewayEvent::Reconnecting { .. } => {}
            e => panic!("unexpected {e:?}"),
        }
    }
    h.close().unwrap();
    assert!(task.await.unwrap().is_ok());
    server.await.unwrap();
}
