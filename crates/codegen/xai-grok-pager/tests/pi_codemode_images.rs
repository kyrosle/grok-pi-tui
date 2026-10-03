use base64::Engine;
use xai_grok_pager::prompt_images::ScrollbackImageRef;
use xai_grok_pager::scrollback::block::BlockContent;
use xai_grok_pager::scrollback::blocks::tool::CodemodeToolCallBlock;

#[test]
fn codemode_images_use_the_native_gallery_and_replay_cache() {
    let mut encoded = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(8, 8)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let data = base64::engine::general_purpose::STANDARD.encode(encoded.into_inner());
    let live = ScrollbackImageRef::from_base64(&data, "image/png").unwrap();
    let replay = ScrollbackImageRef::from_base64(&data, "image/png").unwrap();
    assert_eq!(live.path, replay.path);
    let mut block = CodemodeToolCallBlock::new("image(result)");
    block.images.push(live);
    assert_eq!(block.image_references().len(), 1);
    assert!(block.image_references()[0].path.is_file());
}

#[test]
fn invalid_tool_images_cannot_create_gallery_items() {
    assert!(ScrollbackImageRef::from_base64("not base64", "image/png").is_none());
    assert!(ScrollbackImageRef::from_base64("PHN2Zz4=", "image/svg+xml").is_none());
}

#[test]
#[ignore = "consume PI_NATIVE_CAPTURE from the installed-Pi adapter projection fixture"]
fn actual_pi_acp_reaches_native_codemode_renderer_and_image_gallery() {
    use xai_grok_pager::acp::{meta::NotificationMeta, tracker::AcpUpdateTracker};
    use xai_grok_pager::scrollback::types::{BlockContext, DisplayMode};
    use xai_grok_pager::scrollback::{
        block::RenderBlock, blocks::tool::ToolCallBlock, state::ScrollbackState,
    };

    let capture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::env::var("PI_NATIVE_CAPTURE").expect("actual ACP capture is required"))
            .unwrap(),
    )
    .unwrap();
    let case = capture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["scenario"] == "codemode")
        .unwrap();
    let mut paths = Vec::new();
    for phase in ["live", "replay"] {
        let mut tracker = AcpUpdateTracker::new();
        let mut state = ScrollbackState::new();
        for event in case[phase].as_array().unwrap() {
            if event["kind"] != "session" {
                continue;
            }
            let notification: agent_client_protocol::SessionNotification =
                serde_json::from_value(event["notification"].clone()).unwrap();
            let meta = NotificationMeta::from_json(notification.meta.as_ref());
            tracker.handle_update(notification.update, &meta, &mut state);
        }
        for index in 0..state.len() {
            if matches!(
                &state.get(index).unwrap().block,
                RenderBlock::ToolCall(ToolCallBlock::Codemode(_))
            ) {
                let id = state.get(index).unwrap().id;
                state.get_by_id_mut(id).unwrap().display_mode = DisplayMode::Expanded;
            }
        }
        let cards: Vec<_> = (0..state.len())
            .filter_map(|index| match &state.get(index).unwrap().block {
                RenderBlock::ToolCall(ToolCallBlock::Codemode(block)) => Some(block),
                _ => None,
            })
            .collect();
        assert_eq!(
            cards.len(),
            1,
            "{phase}: one native Codemode card, no duplicated nested rows"
        );
        let card = cards[0];
        assert_eq!(card.calls.len(), 1);
        assert_eq!(card.image_references().len(), 1);
        let path = &card.image_references()[0].path;
        assert_eq!(image::open(path).unwrap().width(), 8);
        let media = card
            .inline_media()
            .expect("native image renderer receives metadata");
        assert_eq!((media.width, media.height), (8, 8));
        assert_eq!(media.path, *path);
        paths.push(path.clone());
        let output = card.output(&BlockContext {
            mode: DisplayMode::Expanded,
            is_running: false,
            width: 120,
            raw: false,
            max_lines: None,
            appearance: Default::default(),
            is_selected: false,
            cwd: None,
        });
        let text = output
            .lines
            .iter()
            .map(|line| {
                line.content
                    .spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("NATIVE_CODEMODE_OUTPUT"),
            "{phase}: native output missing: {text}"
        );
        assert!(
            text.contains("fixture_note"),
            "{phase}: native nested call missing: {text}"
        );
        if card.inline_open_button().is_some() {
            assert!(
                text.contains("[Open Image]"),
                "{phase}: native open line is not painted"
            );
            state.prepare_layout(120, 40);
            let area = ratatui::layout::Rect::new(0, 0, 120, 40);
            let mut buffer = ratatui::buffer::Buffer::empty(area);
            let frame = xai_grok_pager::scrollback::render::render_scrolled_entries_with_scratch(
                &mut buffer,
                area,
                &state.entries_in_range(0..state.len()),
                0,
                None,
                &xai_grok_pager::theme::Theme::current(),
                state.appearance(),
                state.get_cached_entry_layouts().unwrap(),
                0,
                None,
                None,
                None,
                0,
                0,
                &[],
                None,
                None,
            );
            let placement = frame
                .inline_media
                .iter()
                .find(|item| {
                    item.info.path == *paths.last().unwrap()
                        && item.open_button_screen_rect.is_some()
                })
                .expect("native image button hit rectangle");
            let button = placement.open_button_screen_rect.unwrap();
            let painted = (button.x..button.right())
                .map(|x| buffer[(x, button.y)].symbol())
                .collect::<String>();
            assert_eq!(
                painted, "[Open Image]",
                "native hit rectangle must match its painted button"
            );
            assert!(
                placement.filepath_screen_rect.is_none(),
                "script row must not become an image-path copy target"
            );
        }
    }
    assert_eq!(
        paths[0], paths[1],
        "live/replay use the same native decoded image cache"
    );
}
