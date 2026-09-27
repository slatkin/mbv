use mbv_components::library_panel::LibraryPanel;
use mbv_ui_model::settings::PanelFocus;

impl super::super::Model {
    pub(super) fn refresh_visual_selection(&mut self) {
        let summary = if self.app.effective_panel_focus() == PanelFocus::Queue {
            self.application
                .get_component(&mbv_ui_msg::ComponentId::Queue)
                .and_then(|component| {
                    component
                        .as_any()
                        .downcast_ref::<mbv_components::QueueComponent>()
                })
                .map(mbv_components::queue::QueueComponent::selection_summary)
        } else {
            self.application
                .get_component_mut(&mbv_ui_msg::ComponentId::Library)
                .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
                .and_then(mbv_components::library_panel::panel::LibraryPanel::focused_summary)
        };
        self.visual_selection = summary
            .filter(|summary| summary.count > 0)
            .map(|summary| (self.app.effective_panel_focus(), summary.count));
    }
}
