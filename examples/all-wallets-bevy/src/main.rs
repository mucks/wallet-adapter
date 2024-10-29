use anyhow::{anyhow, Result};
use bevy::prelude::*;
use std::sync::{Arc, OnceLock, RwLock, RwLockWriteGuard};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(WalletAdapterBevyPlugin)
        .run();
}

pub struct WalletAdapterBevyPlugin;

impl Plugin for WalletAdapterBevyPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<WalletEvent>();
        app.insert_resource(Wallet { info: None });
        app.add_systems(Startup, setup_wallet_menu);
        app.add_systems(
            Update,
            (
                wallet_menu_interaction_system,
                wallet_event_system,
                wallet_menu_system,
                async_wallet_event_system,
            ),
        );
    }
}

static ASYNC_WALLET_EVENT_QUEUE: OnceLock<Arc<RwLock<Vec<AsyncWalletEvent>>>> = OnceLock::new();

// This is a workaround to catch the async wallet event in the main thread
struct AsyncWalletEventQueue;

impl AsyncWalletEventQueue {
    fn get_rw_lock() -> Result<RwLockWriteGuard<'static, Vec<AsyncWalletEvent>>> {
        ASYNC_WALLET_EVENT_QUEUE
            .get_or_init(|| Arc::new(RwLock::new(vec![])))
            .write()
            .map_err(|err| anyhow!("{:?}", err))
    }

    fn push(event: AsyncWalletEvent) -> Result<()> {
        let mut wallet_event_queue = Self::get_rw_lock()?;
        wallet_event_queue.push(event);
        Ok(())
    }

    fn pop() -> Result<Option<AsyncWalletEvent>> {
        let mut wallet_event_queue = Self::get_rw_lock()?;
        Ok(wallet_event_queue.pop())
    }

    fn _clear() -> Result<()> {
        let mut wallet_event_queue = Self::get_rw_lock()?;
        wallet_event_queue.clear();
        Ok(())
    }
}

#[derive(Debug, Resource)]
pub struct Wallet {
    pub info: Option<WalletInfo>,
}

#[derive(Debug)]
pub struct WalletInfo {
    pub amount: u32,
    pub address: String,
}

#[derive(Debug, Event)]
pub enum WalletEvent {
    ConnectBtnClick,
    DisconnectBtnClick,
    Connected,
    Disconnected,
}

pub enum AsyncWalletEvent {
    ConnectionCompleted(Result<String>),
}

#[derive(Debug, Component)]
pub enum WalletButtonType {
    Connect,
    Disconnect,
}

#[derive(Debug, Component)]
pub struct WalletMenu;

const NORMAL_BUTTON: Color = Color::linear_rgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::linear_rgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::linear_rgb(0.35, 0.75, 0.35);

fn async_wallet_event_system(mut ev_writer: EventWriter<WalletEvent>, mut wallet: ResMut<Wallet>) {
    if let Ok(Some(event)) = AsyncWalletEventQueue::pop() {
        match event {
            AsyncWalletEvent::ConnectionCompleted(result) => match result {
                Ok(address) => {
                    debug!("WalletEvent::ConnectionCompleted: {:?}", address);
                    wallet.info = Some(WalletInfo { amount: 0, address });
                    ev_writer.send(WalletEvent::Connected);
                }
                Err(err) => {
                    debug!("WalletEvent::ConnectionCompleted: {:?}", err);
                }
            },
        }
    }
}

fn wallet_menu_system(
    mut ev_reader: EventReader<WalletEvent>,
    mut wallet_menu_query: Query<&mut Text, (With<WalletMenu>, Without<ConnectDisconnectBtnText>)>,
    mut wallet: ResMut<Wallet>,
    mut toggle_connect_btn: Query<&mut WalletButtonType, With<WalletButtonType>>,
    mut toggle_connect_btn_text: Query<
        &mut Text,
        (With<ConnectDisconnectBtnText>, Without<WalletMenu>),
    >,
) {
    for event in ev_reader.read() {
        match event {
            WalletEvent::Connected => {
                debug!("WalletEvent::Connected");
                if let Some(info) = &wallet.info {
                    wallet_menu_query.single_mut().sections[0].value = info.address.clone();
                }
                toggle_connect_btn_text.single_mut().sections[0].value = "Disconnect".to_string();
                *toggle_connect_btn.single_mut() = WalletButtonType::Disconnect;
            }
            WalletEvent::DisconnectBtnClick => {
                debug!("WalletEvent::DisconnectBtnClick");
                wallet.info = None;
                wallet_menu_query.single_mut().sections[0].value = String::new();
                toggle_connect_btn_text.single_mut().sections[0].value = "Connect".to_string();
                *toggle_connect_btn.single_mut() = WalletButtonType::Connect;
            }
            _ => {}
        }
    }
}

fn wallet_event_system(
    mut _commands: Commands,
    mut ev_reader: EventReader<WalletEvent>,
    mut _wallet: ResMut<Wallet>,
) {
    for event in ev_reader.read() {
        if let WalletEvent::ConnectBtnClick = event {
            debug!("WalletEvent::ConnectBtnClick");
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn wallet_menu_interaction_system(
    mut interaction_query: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            &WalletButtonType,
        ),
        (Changed<Interaction>, With<WalletButtonType>),
    >,
    mut ev_writer: EventWriter<WalletEvent>,
) {
    for (interaction, mut color, mut border_color, button_type) in &mut interaction_query {
        // styling

        match *interaction {
            Interaction::Pressed => {
                *color = PRESSED_BUTTON.into();
                border_color.0 = Color::linear_rgb(255., 0., 0.);
            }
            Interaction::Hovered => {
                *color = HOVERED_BUTTON.into();
                border_color.0 = Color::WHITE;
            }
            Interaction::None => {
                *color = NORMAL_BUTTON.into();
                border_color.0 = Color::BLACK;
            }
        }

        match *interaction {
            Interaction::Pressed => match button_type {
                WalletButtonType::Connect => {
                    println!("Connect button clicked");
                    ev_writer.send(WalletEvent::ConnectBtnClick);
                }
                WalletButtonType::Disconnect => {
                    println!("Disconnect button clicked");
                    ev_writer.send(WalletEvent::DisconnectBtnClick);
                }
            },
            Interaction::Hovered => {
                *color = HOVERED_BUTTON.into();
                border_color.0 = Color::WHITE;
            }
            _ => {}
        }
    }
}

#[derive(Debug, Component)]
pub struct ConnectDisconnectBtnText;

pub fn setup_wallet_menu(mut commands: Commands) {
    // setup camera
    commands.spawn(Camera2dBundle::default());

    // setup connect button
    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            // spawn text view for wallet
            parent
                .spawn(TextBundle::from_section(
                    "",
                    TextStyle {
                        font_size: 40.0,
                        color: Color::linear_rgb(0.9, 0.9, 0.9),
                        ..Default::default()
                    },
                ))
                .insert(WalletMenu);

            // spawn connect button
            parent
                .spawn(ButtonBundle {
                    style: Style {
                        width: Val::Px(150.0),
                        height: Val::Px(65.0),
                        border: UiRect::all(Val::Px(5.0)),
                        // horizontally center child text
                        justify_content: JustifyContent::Center,
                        // vertically center child text
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    border_color: BorderColor(Color::BLACK),
                    background_color: NORMAL_BUTTON.into(),
                    ..default()
                })
                .with_children(|parent| {
                    parent
                        .spawn(TextBundle::from_section(
                            "Connect",
                            TextStyle {
                                font_size: 40.0,
                                color: Color::linear_rgb(0.9, 0.9, 0.9),
                                ..Default::default()
                            },
                        ))
                        .insert(ConnectDisconnectBtnText);
                })
                .insert(WalletButtonType::Connect);
        });

    // setup address display
    // setup balance display
}
