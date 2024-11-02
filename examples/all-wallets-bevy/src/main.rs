use bevy::prelude::*;
use wallet_adapter_bevy::WalletAdapterPlugin;
use wallet_adapter_unsafe_burner::UnsafeBurnerWallet;
use wallet_adapter_unsafe_persistent::UnsafePersistentWallet;
use wallet_adapter_x86::storage::X86Storage;

fn main() {
    let unsafe_burner = UnsafeBurnerWallet::new();
    let unsafe_persistent =
        UnsafePersistentWallet::new(X86Storage::new("all-wallets-bevy").unwrap()).unwrap();

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(WalletAdapterPlugin {
            active_wallet: Box::new(unsafe_persistent.clone()),
            wallets: vec![Box::new(unsafe_burner), Box::new(unsafe_persistent)],
        })
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    // setup camera
    commands.spawn(Camera2dBundle::default());
}
