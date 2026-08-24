use super::types_settings::{
    ServiceActionIntent, ServiceEntry, SettingsDestination, SERVICE_ENTRIES,
};
use super::App;
use super::{ConfirmAction, ConfirmModal};
use mbv_core::service_runtime::ServiceState;

pub(super) struct EmbySetupForm {
    pub(super) fields: [String; 3],
    pub(super) focus: usize,
    pub(super) busy: bool,
    pub(super) error: String,
    pub(super) generation: Option<mbv_core::service_runtime::SetupGeneration>,
    pub(super) previous_state: ServiceState,
}

pub(super) struct AudiobookshelfSetupForm {
    pub(super) fields: [String; 2],
    pub(super) focus: usize,
    pub(super) busy: bool,
    pub(super) error: String,
    pub(super) generation: Option<mbv_core::service_runtime::SetupGeneration>,
    pub(super) previous_state: ServiceState,
}

impl EmbySetupForm {
    fn new(config: &crate::config::Config, previous_state: ServiceState) -> Self {
        Self {
            fields: [
                config
                    .emby_setup
                    .as_ref()
                    .map(|setup| setup.server_url.clone())
                    .unwrap_or_else(|| config.server_url.trim_end_matches('/').to_string()),
                String::new(),
                String::new(),
            ],
            focus: 1,
            busy: false,
            error: String::new(),
            generation: None,
            previous_state,
        }
    }
}

impl App {
    pub(crate) fn open_services_settings(&mut self) {
        self.request_sidebar_open(super::SidebarId::Settings);
        self.settings_destination = SettingsDestination::Services;
        self.services_cursor = self.services_cursor.min(SERVICE_ENTRIES.len() - 1);
    }

    fn open_emby_setup(&mut self) {
        let previous = self.emby_runtime.state;
        let config = self.config.lock().unwrap().clone();
        self.emby_setup_form = Some(EmbySetupForm::new(&config, previous));
    }

    fn open_audiobookshelf_setup(&mut self) {
        let config = self.config.lock().unwrap().clone();
        self.audiobookshelf_setup_form = Some(AudiobookshelfSetupForm {
            fields: [
                config
                    .audiobookshelf_setup
                    .as_ref()
                    .map_or_else(String::new, |s| s.server_url.clone()),
                String::new(),
            ],
            focus: 1,
            busy: false,
            error: String::new(),
            generation: None,
            previous_state: self.audiobookshelf_runtime.state,
        });
    }

    pub(super) fn cancel_emby_setup(&mut self) {
        if let Some(form) = self.emby_setup_form.take() {
            if let Some(generation) = form.generation {
                self.emby_runtime
                    .cancel_setup(generation, form.previous_state);
            }
        }
        self.emby_setup_rx = None;
    }

    pub(super) fn cancel_audiobookshelf_setup(&mut self) {
        if let Some(mut form) = self.audiobookshelf_setup_form.take() {
            form.fields[1].clear();
            if let Some(generation) = form.generation {
                self.audiobookshelf_runtime
                    .cancel_setup(generation, form.previous_state);
            }
        }
        self.audiobookshelf_setup_rx = None;
    }

    pub(super) fn submit_audiobookshelf_setup(&mut self) {
        let Some(form) = self.audiobookshelf_setup_form.as_mut() else {
            return;
        };
        if form.busy {
            return;
        }
        let server_url = form.fields[0].trim().trim_end_matches('/').to_string();
        if server_url.is_empty() || form.fields[1].trim().is_empty() {
            form.error = "Server URL and API key are required".into();
            form.fields[1].clear();
            form.focus = if server_url.is_empty() { 0 } else { 1 };
            return;
        }
        form.fields[0] = server_url.clone();
        let api_key = std::mem::take(&mut form.fields[1]);
        let generation = self.audiobookshelf_runtime.begin_setup();
        form.generation = Some(generation);
        form.busy = true;
        form.error = "Validating Audiobookshelf setup…".into();
        self.audiobookshelf_setup_rx = Some(super::service_startup::start_audiobookshelf_setup(
            server_url,
            api_key,
            generation,
            form.previous_state,
        ));
    }

    pub(super) fn handle_emby_setup_worker_disconnect(&mut self) {
        let Some(form) = self.emby_setup_form.as_mut() else {
            self.emby_setup_rx = None;
            return;
        };
        if let Some(generation) = form.generation {
            self.emby_runtime
                .cancel_setup(generation, form.previous_state);
        } else {
            self.emby_runtime.state = form.previous_state;
        }
        form.busy = false;
        form.fields[2].clear();
        form.error = "Emby setup stopped unexpectedly; check the server and retry".into();
        self.emby_setup_rx = None;
    }

    pub(super) fn submit_emby_setup(&mut self) {
        let Some(form) = self.emby_setup_form.as_mut() else {
            return;
        };
        if form.busy {
            return;
        }
        let server_url = form.fields[0].trim().trim_end_matches('/').to_string();
        let username = form.fields[1].trim().to_string();
        if server_url.is_empty() || username.is_empty() || form.fields[2].is_empty() {
            form.error = "Server URL, username, and password are required".into();
            form.fields[2].clear();
            form.focus = if server_url.is_empty() {
                0
            } else if username.is_empty() {
                1
            } else {
                2
            };
            return;
        }
        form.fields[0] = server_url.clone();
        form.fields[1] = username.clone();
        let password = std::mem::take(&mut form.fields[2]);
        let generation = self.emby_runtime.begin_setup();
        form.generation = Some(generation);
        form.busy = true;
        form.error = "Validating Emby setup…".into();
        let previous = form.previous_state;
        let config = self.config.lock().unwrap().clone();
        self.emby_setup_rx = Some(super::service_startup::start_setup(
            config, server_url, username, password, generation, previous,
        ));
    }

    pub(super) fn activate_service_entry(&mut self) {
        let Some(&entry) = SERVICE_ENTRIES.get(self.services_cursor) else {
            return;
        };
        let intent = match entry {
            ServiceEntry::Emby => match self.emby_runtime.state {
                ServiceState::NotConfigured | ServiceState::NeedsAuthentication => {
                    ServiceActionIntent::SetupEmby
                }
                ServiceState::Connecting => return,
                ServiceState::Ready => ServiceActionIntent::RepairEmby,
                ServiceState::Unavailable => ServiceActionIntent::RetryEmby,
            },
            ServiceEntry::Audiobookshelf => match self.audiobookshelf_runtime.state {
                ServiceState::NotConfigured | ServiceState::NeedsAuthentication => {
                    ServiceActionIntent::SetupAudiobookshelf
                }
                ServiceState::Connecting => return,
                ServiceState::Ready => ServiceActionIntent::SetupAudiobookshelf,
                ServiceState::Unavailable => ServiceActionIntent::TestAudiobookshelf,
            },
            ServiceEntry::Feeds => ServiceActionIntent::ManageFeeds,
        };
        self.route_service_action(intent);
    }

    pub(super) fn route_service_action(&mut self, intent: ServiceActionIntent) {
        match intent {
            ServiceActionIntent::ManageFeeds => {
                self.pending_overlay = Some(super::types_overlay::OverlayRequest::OpenFeedsManage);
            }
            ServiceActionIntent::SetupAudiobookshelf => self.open_audiobookshelf_setup(),
            ServiceActionIntent::TestAudiobookshelf => self.test_audiobookshelf_connection(),
            ServiceActionIntent::RemoveAudiobookshelf => self.request_audiobookshelf_removal(),
            ServiceActionIntent::ReplaceAudiobookshelf => self.open_audiobookshelf_setup(),
            ServiceActionIntent::SetupEmby => self.open_emby_setup(),
            ServiceActionIntent::RepairEmby => self.open_emby_setup(),
            ServiceActionIntent::RetryEmby => self.retry_emby(),
        }
    }

    pub(super) fn service_entry_name(entry: ServiceEntry) -> &'static str {
        match entry {
            ServiceEntry::Emby => "Emby",
            ServiceEntry::Audiobookshelf => "Audiobookshelf",
            ServiceEntry::Feeds => "Feeds",
        }
    }

    pub(super) fn request_emby_removal(&mut self) {
        if self.emby_runtime.state == ServiceState::NotConfigured {
            return;
        }
        self.ask_confirm(ConfirmModal {
            title: " Remove Emby ".into(),
            message: "Remove Emby? Service-owned setup and state will be cleared.".into(),
            hint: "[y/Enter] Confirm    [Esc] Cancel".into(),
            on_confirm: ConfirmAction::RemoveEmby,
        });
    }

    pub(super) fn request_audiobookshelf_removal(&mut self) {
        if self.audiobookshelf_runtime.state == ServiceState::NotConfigured {
            return;
        }
        self.ask_confirm(ConfirmModal {
            title: " Remove Audiobookshelf ".into(),
            message: "Remove Audiobookshelf? Service-owned setup and state will be cleared.".into(),
            hint: "[y/Enter] Confirm    [Esc] Cancel".into(),
            on_confirm: ConfirmAction::RemoveAudiobookshelf,
        });
    }

    pub(super) fn service_state_label(&self, entry: ServiceEntry) -> &'static str {
        match entry {
            ServiceEntry::Emby => service_state_label(self.emby_runtime.state),
            ServiceEntry::Audiobookshelf => service_state_label(self.audiobookshelf_runtime.state),
            ServiceEntry::Feeds => "Always present",
        }
    }

    pub(super) fn service_action_label(&self, entry: ServiceEntry) -> &'static str {
        match entry {
            ServiceEntry::Emby => match self.emby_runtime.state {
                ServiceState::NotConfigured | ServiceState::NeedsAuthentication => "Set up Emby",
                ServiceState::Connecting => "",
                ServiceState::Ready => "Repair / replace (d removes)",
                ServiceState::Unavailable => "Retry connection",
            },
            ServiceEntry::Audiobookshelf => match self.audiobookshelf_runtime.state {
                ServiceState::NotConfigured => "Set up Audiobookshelf",
                ServiceState::Connecting => "",
                ServiceState::Ready => "Test (t) · Repair · Replace (r) · Remove (d)",
                ServiceState::NeedsAuthentication => "Repair · Replace (r) · Remove (d)",
                ServiceState::Unavailable => "Test (t) · Repair · Replace (r) · Remove (d)",
            },
            ServiceEntry::Feeds => "Manage feeds",
        }
    }

    fn retry_emby(&mut self) {
        if self.emby_runtime.state != ServiceState::Unavailable {
            return;
        }
        let config = self.config.lock().unwrap().clone();
        if config.emby_setup.is_none() {
            self.emby_runtime.state = ServiceState::NotConfigured;
            self.open_emby_setup();
            return;
        }
        if mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Emby).is_none() {
            self.emby_runtime.state = ServiceState::NeedsAuthentication;
            self.open_emby_setup();
            return;
        }
        let generation = self.emby_runtime.begin_retry();
        self.emby_startup_rx = Some(super::service_startup::start(config, generation));
    }

    pub(super) fn service_context(&self, entry: ServiceEntry) -> String {
        match entry {
            ServiceEntry::Feeds => match self.config.lock().unwrap().feeds.len() {
                0 => "No subscriptions".into(),
                n => format!("{n} subscription{}", if n == 1 { "" } else { "s" }),
            },
            _ => String::new(),
        }
    }
}

fn service_state_label(state: ServiceState) -> &'static str {
    match state {
        ServiceState::NotConfigured => "Not configured",
        ServiceState::Connecting => "Connecting",
        ServiceState::Ready => "Ready",
        ServiceState::NeedsAuthentication => "Needs authentication",
        ServiceState::Unavailable => "Unavailable",
    }
}
