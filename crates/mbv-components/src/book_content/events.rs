use super::{
    AudiobookshelfBookIntent, BookContent, LibrarySlotEvent, MediaListOperation,
    MediaListSurfaceInput, Msg, ShellRequest,
};
use crate::media_list::RowIntent;

impl BookContent {
    pub fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::SelectorPicked(index) => {
                self.select_bucket(index);
                Some(self.bucket_request())
            }
            LibrarySlotEvent::List(input) => self.book_list_event(input),
            LibrarySlotEvent::WorkspaceSelectorPicked(_) => None,
            LibrarySlotEvent::HeroActivate => Some(Msg::Shell(Box::new(
                ShellRequest::AudiobookshelfBookIntent(if self.chapter_focused {
                    AudiobookshelfBookIntent::ActivateChapter(self.chapter_target())
                } else {
                    AudiobookshelfBookIntent::Activate
                }),
            ))),
            LibrarySlotEvent::HeroPane(input) => self.chapter_list_event(input),
        }
    }

    fn book_list_event(&mut self, input: MediaListSurfaceInput) -> Option<Msg> {
        match input {
            MediaListSurfaceInput::Wheel { at, delta } => self
                .carrier
                .claims_current_point(at)
                .then(|| self.move_book(MediaListSurfaceInput::Wheel { at, delta })),
            MediaListSurfaceInput::Click(at)
            | MediaListSurfaceInput::ToggleClick(at)
            | MediaListSurfaceInput::RangeClick(at) => {
                let target = self.carrier.resolve_current_point(at)?.clone();
                self.carrier.delegate_operation(
                    input
                        .into_operation(Some(target))
                        .expect("resolved media-list pointer target"),
                );
                let () = ();
                self.sync_book_from_owner();
                Some(self.book_request())
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
            MediaListSurfaceInput::ContextClick(at) => {
                // Right-click resolves through the shared owner the way the
                // `.` keyboard gesture does: a Visual multi-selection
                // supplies every marked row in display order, otherwise the
                // clicked row.
                let target = self.carrier.resolve_current_point(at)?.clone();
                let outcome = self
                    .carrier
                    .delegate_operation(MediaListOperation::Context(target.clone()));
                let targets = match outcome.external_intent {
                    Some(RowIntent::ContextSelection(targets)) => targets,
                    Some(RowIntent::Context(target)) => vec![target],
                    _ => vec![target],
                };
                Some(Self::book_context_msg(targets, Some((at.x, at.y))))
            }
            _ => None,
        }
    }

    fn chapter_list_event(&mut self, input: MediaListSurfaceInput) -> Option<Msg> {
        match input {
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
                Some(self.chapter_focus_request())
            }
            MediaListSurfaceInput::DoubleClick(at) | MediaListSurfaceInput::ContextClick(at) => {
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
        }
    }
}
