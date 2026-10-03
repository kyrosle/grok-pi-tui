use super::{bounded_clipboard_probe, clipboard_probe_stage, probe_clipboard_attachment_blocking};
use crate::app::actions::{ClipboardPasteSource, ClipboardTextRead, ProbedAttachment};
use crate::clipboard::{ClipboardProbeDropReason as Reason, ProbeDrop};
use std::time::Duration;

#[tokio::test]
async fn deadline_and_panic_complete_without_waiting_for_the_worker() {
    let started = std::time::Instant::now();
    let late = clipboard_probe_stage(Duration::from_millis(20), || {
        std::thread::sleep(Duration::from_millis(300));
        Ok((ProbedAttachment::NoRaster, None))
    })
    .await;
    assert!(started.elapsed() < Duration::from_millis(250));
    assert_eq!(late.expect_err("deadline expires").reason, Reason::Timeout);
    let panicked = clipboard_probe_stage(Duration::from_secs(5), || panic!("decoder failed")).await;
    assert_eq!(
        panicked.expect_err("panic completes").reason,
        Reason::Panicked
    );
}

#[tokio::test]
async fn every_drop_reason_returns_a_native_completion_and_keeps_failure_text() {
    for reason in [
        Reason::ReadFailed,
        Reason::Timeout,
        Reason::Panicked,
        Reason::PersistFailed,
        Reason::PasteboardChangedBeforeRead,
        Reason::PasteboardChangedAfterRead,
        Reason::BracketedPayloadMismatch,
        Reason::BracketedOriginReadFailed,
    ] {
        let (attachment, files) = bounded_clipboard_probe(Duration::from_secs(5), move || {
            Err(ProbeDrop {
                reason,
                image: None,
                message: Some("disk full".into()),
            })
        })
        .await;
        match reason {
            Reason::ReadFailed | Reason::Timeout | Reason::Panicked => {
                assert!(matches!(attachment, ProbedAttachment::ProbeFailed))
            }
            Reason::PersistFailed => assert!(
                matches!(attachment, ProbedAttachment::PersistFailed(ref text) if text == "disk full")
            ),
            _ => assert!(matches!(attachment, ProbedAttachment::ProbeDropped)),
        }
        assert!(files.is_none());
    }
}

#[test]
fn mismatched_bracketed_text_drops_before_reading_an_unrelated_image() {
    crate::clipboard::set_clipboard_probe_hook(crate::clipboard::ClipboardProbeHook {
        text: Some("clipboard caption".into()),
        snapshot: Some((Some(1), true)),
        ..Default::default()
    });
    let outcome =
        probe_clipboard_attachment_blocking(Some(1), Some("IME commit".into()), true, None);
    let reads = crate::clipboard::clipboard_probe_call_count();
    crate::clipboard::clear_clipboard_probe_hook();
    assert_eq!(
        outcome
            .expect_err("unrelated image stays on pasteboard")
            .reason,
        Reason::BracketedPayloadMismatch
    );
    assert_eq!(reads, 0);
}

#[test]
fn discarded_or_failed_probe_preserves_captured_caption_without_repeating_inserted_text() {
    let source = ClipboardPasteSource::ClipboardKey {
        text: ClipboardTextRead::Success(Some("caption".into())),
        tip_showing: false,
    };
    let inserted = ClipboardPasteSource::BracketedInserted {
        text: "caption".into(),
        insertion: crate::app::actions::ClipboardTextInsertion::Inserted,
    };
    for attachment in [
        ProbedAttachment::NoRaster,
        ProbedAttachment::ProbeDropped,
        ProbedAttachment::ProbeFailed,
    ] {
        assert_eq!(source.text_to_insert_on_miss(&attachment), Some("caption"));
        assert_eq!(inserted.text_to_insert_on_miss(&attachment), None);
    }
    assert_eq!(
        source.text_to_insert_on_miss(&ProbedAttachment::PersistFailed("disk full".into())),
        None
    );
}
