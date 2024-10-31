use anyhow::Result;
use bevy::prelude::*;
use wallet_adapter_base::{BaseWalletAdapter, WalletAdapterEvent};
use wallet_adapter_unsafe_burner::UnsafeBurnerWallet;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(WalletAdapterBevyPlugin)
        .run();
}

pub struct WalletAdapterBevyPlugin;

impl Plugin for WalletAdapterBevyPlugin {
    fn build(&self, app: &mut App) {
        let burner_wallet = UnsafeBurnerWallet::new();

        app.add_event::<WalletEvent>();
        app.add_event::<WalletUiEvent>();

        app.insert_resource(Wallet {
            active_wallet: Box::new(burner_wallet.clone()),
            wallets: vec![Box::new(burner_wallet)],
        });
        app.add_systems(Startup, setup_wallet_menu);
        app.add_systems(
            Update,
            (
                wallet_menu_interaction_system,
                wallet_event_system,
                wallet_menu_system,
                on_wallet_event_system,
            ),
        );
    }
}

#[derive(Resource)]
pub struct Wallet {
    pub active_wallet: Box<dyn BaseWalletAdapter + Sync + Send>,
    pub wallets: Vec<Box<dyn BaseWalletAdapter + Sync + Send>>,
}

#[derive(Debug, Event)]
pub enum WalletEvent {
    Connected(String),
    Disconnected,
}

#[derive(Debug, Event)]
pub enum WalletUiEvent {
    ConnectBtnClick,
    DisconnectBtnClick,
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

fn wallet_menu_system(
    mut ev_reader: EventReader<WalletEvent>,
    mut wallet_menu_query: Query<&mut Text, (With<WalletMenu>, Without<ConnectDisconnectBtnText>)>,
    mut toggle_connect_btn: Query<&mut WalletButtonType, With<WalletButtonType>>,
    mut toggle_connect_btn_text: Query<
        &mut Text,
        (With<ConnectDisconnectBtnText>, Without<WalletMenu>),
    >,
) {
    for event in ev_reader.read() {
        match event {
            WalletEvent::Connected(addr) => {
                debug!("WalletEvent::Connected");
                let addr_short = format!("{}..{}", &addr[0..4], &addr[addr.len() - 4..]);
                wallet_menu_query.single_mut().sections[0].value = addr_short.clone();
                toggle_connect_btn_text.single_mut().sections[0].value = "Disconnect".to_string();
                *toggle_connect_btn.single_mut() = WalletButtonType::Disconnect;
            }
            WalletEvent::Disconnected => {
                debug!("WalletEvent::Disconnect");
                wallet_menu_query.single_mut().sections[0].value = String::new();
                toggle_connect_btn_text.single_mut().sections[0].value = "Connect".to_string();
                *toggle_connect_btn.single_mut() = WalletButtonType::Connect;
            }
        }
    }
}

fn on_wallet_event_system(mut ev_writer: EventWriter<WalletEvent>, wallet: Res<Wallet>) {
    let active_wallet = wallet.active_wallet.clone();

    if let Some(ev) = active_wallet.event_emitter().try_recv() {
        info!("on_wallet_event_system: {:?}", ev);

        match ev {
            WalletAdapterEvent::Connect(addr) => {
                ev_writer.send(WalletEvent::Connected(addr.to_string()));
            }
            WalletAdapterEvent::Disconnect => {
                ev_writer.send(WalletEvent::Disconnected);
            }
            _ => {}
        }
    }
}

fn wallet_event_system(
    mut _commands: Commands,
    mut ev_reader: EventReader<WalletUiEvent>,
    wallet: Res<Wallet>,
) {
    for event in ev_reader.read() {
        match event {
            WalletUiEvent::ConnectBtnClick => {
                debug!("WalletEvent::ConnectBtnClick");

                let mut active_wallet = wallet.active_wallet.clone();

                let other_task = async move {
                    active_wallet.connect().await.unwrap();
                };
                futures::executor::block_on(other_task);
            }
            WalletUiEvent::DisconnectBtnClick => {
                debug!("WalletEvent::DisconnectBtnClick");

                let active_wallet = wallet.active_wallet.clone();

                let other_task = async move {
                    active_wallet.disconnect().await.unwrap();
                };
                futures::executor::block_on(other_task);
            }
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
    mut ev_writer: EventWriter<WalletUiEvent>,
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
                    ev_writer.send(WalletUiEvent::ConnectBtnClick);
                }
                WalletButtonType::Disconnect => {
                    println!("Disconnect button clicked");
                    ev_writer.send(WalletUiEvent::DisconnectBtnClick);
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
                align_items: AlignItems::End,
                justify_content: JustifyContent::Start,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ..default()
        })
        .with_children(|parent| {
            // spawn connect button
            parent
                .spawn(ButtonBundle {
                    style: Style {
                        width: Val::Px(200.0),
                        height: Val::Px(50.0),
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
                            "Connect Wallet",
                            TextStyle {
                                font_size: 25.0,
                                color: Color::linear_rgb(0.9, 0.9, 0.9),
                                ..Default::default()
                            },
                        ))
                        .insert(ConnectDisconnectBtnText);
                })
                .insert(WalletButtonType::Connect);
            // spawn text view for wallet
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Px(200.0),
                        height: Val::Px(50.0),
                        border: UiRect::all(Val::Px(5.0)),
                        // horizontally center child text
                        justify_content: JustifyContent::Center,
                        // vertically center child text
                        align_items: AlignItems::Center,
                        margin: UiRect {
                            top: Val::Px(10.0),
                            ..default()
                        },
                        ..default()
                    },
                    border_color: BorderColor(Color::BLACK),
                    ..default()
                })
                .with_children(|parent| {
                    parent
                        .spawn(TextBundle::from_section(
                            "",
                            TextStyle {
                                font_size: 30.0,
                                color: Color::linear_rgb(0.9, 0.9, 0.9),
                                ..Default::default()
                            },
                        ))
                        .insert(WalletMenu);
                });
        });

    // setup address display
    // setup balance display
}