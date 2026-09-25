use super::*;

impl BookContent {
    pub(super) fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::SelectorPicked(index) => {
                self.select_bucket(index);
                self.bucket_request()
            }
            LibrarySlotEvent::List(input) => match input {
                MediaListSurfaceInput::Wheel { at, delta } => {
                    if self.carrier.claims_current_point(at) {
                        self.move_book(MediaListSurfaceInput::Wheel { at, delta })
                    } else {
                        None
                    }
                }
                MediaListSurfaceInput::Click(at)
                | MediaListSurfaceInput::ToggleClick(at)
                | MediaListSurfaceInput::RangeClick(at) => {
                    let target = self.carrier.resolve_current_point(at)?.clone();
                    self.carrier.delegate_operation(
                        input
                            .into_operation(Some(target))
                            .expect("resolved media-list pointer target"),
                    );
                    let _ = ();
                    self.sync_book_from_owner();
                    self.book_request()
                }
                MediaListSurfaceInput::DoubleClick(at) => {
                    let target = self.carrier.resolve_current_point(at)?.clone();
                    self.carrier.delegate_operation(
                        MediaListSurfaceInput::Click(at)
                            .into_operation(Some(target))
                            .expect("resolved media-list pointer target"),
                    );
                    self.sync_book_from_owner();
                    Some(Msg::Shell(Box::new(
                        ShellRequest::AudiobookshelfBookIntent(AudiobookshelfBookIntent::Activate),
                    )))
                }
                _ => None,
            },
            LibrarySlotEvent::WorkspaceSelectorPicked(_) => None,
            LibrarySlotEvent::HeroActivate => Some(Msg::Shell(Box::new(
                ShellRequest::AudiobookshelfBookIntent(if self.chapter_focused {
                    AudiobookshelfBookIntent::ActivateChapter(self.chapter_target())
                } else {
                    AudiobookshelfBookIntent::Activate
                }),
            ))),
            LibrarySlotEvent::HeroPane(input) => match input {
                MediaListSurfaceInput::Wheel { at, delta } => {
                    if self.chapter_list.claims_current_point(at) {
                        self.chapter_list.delegate_operation(
                            MediaListSurfaceInput::Wheel { at, delta }
                                .into_operation(None)
                                .expect("resolved media-list pointer target"),
                        );
                    }
                    None
                }
                MediaListSurfaceInput::Click(at)
                | MediaListSurfaceInput::ToggleClick(at)
                | MediaListSurfaceInput::RangeClick(at) => {
                    let target = self.chapter_list.resolve_current_point(at).copied()?;
                    self.enter_chapter_focus();
                    self.chapter_list.delegate_operation(
                        input
                            .into_operation(Some(target))
                            .expect("resolved media-list pointer target"),
                    );
                    self.chapter_focus_request()
                }
                MediaListSurfaceInput::DoubleClick(at)
                | MediaListSurfaceInput::ContextClick(at) => {
                    let target = self.chapter_list.resolve_current_point(at).copied()?;
                    self.enter_chapter_focus();
                    self.chapter_list.delegate_operation(
                        MediaListSurfaceInput::Click(at)
                            .into_operation(Some(target))
                            .expect("resolved media-list pointer target"),
                    );
                    Some(Msg::Shell(Box::new(
                        ShellRequest::AudiobookshelfBookIntent(
                            AudiobookshelfBookIntent::ActivateChapter(self.chapter_target()),
                        ),
                    )))
                }
                _ => None,
            },
        }
    }
}
