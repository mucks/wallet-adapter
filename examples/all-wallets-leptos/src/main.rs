use leptos::*;
use wallet_adapter_base::BaseWalletAdapter;
use wallet_adapter_phantom::PhantomWalletAdapter;
use wallet_adapter_solflare::SolflareWalletAdapter;

#[derive(Clone)]
pub struct Wallets {
    pub wallets: Vec<Box<dyn BaseWalletAdapter>>,
}

impl Wallets {
    pub fn active_wallet(&self, wallet_name: &str) -> Box<dyn BaseWalletAdapter> {
        self.wallets
            .iter()
            .find(|wallet| wallet.name() == wallet_name)
            .cloned()
            .expect("Wallet not found")
    }
}

#[component]
pub fn WalletProvider(
    children: Children,
    wallets: Vec<Box<dyn BaseWalletAdapter>>,
) -> impl IntoView {
    view! {
        <Provider<Wallets> value=Wallets { wallets }>
            {children()}
        </Provider<Wallets>>
    }
}

pub fn use_wallet(active_wallet: &str) -> Box<dyn BaseWalletAdapter> {
    let wallets = use_context::<Wallets>().expect("No WalletContext found");
    wallets.active_wallet(&active_wallet)
}

#[component]
pub fn WalletConnectBtn() -> impl IntoView {
    let active_wallet = use_context::<ReadSignal<String>>().unwrap();

    let wallet = move || use_wallet(&active_wallet.get());

    view! {
        <button on:click=move |_| {
            let w = wallet.clone();
            spawn_local(async move {
                w().connect().await.unwrap();
            });
        }>
            {"Connect"}
        </button>
    }
}

#[component]
pub fn WalletView() -> impl IntoView {
    let active_wallet = use_context::<ReadSignal<String>>().unwrap();
    let active_wallet_name = move || active_wallet.get();

    let wallet = move || use_wallet(&active_wallet_name());
    let wallet_name = move || wallet().name();
    let wallet_pk = move || match wallet().public_key() {
        Some(pk) => pk.to_string(),
        None => "No pubkey".to_string(),
    };

    view! {
        <div>
            <h1>{wallet_name}</h1>
            <p>{wallet_pk}</p>
        </div>
    }
}

#[component]
pub fn WalletSelect(set_active_wallet: WriteSignal<String>) -> impl IntoView {
    let wallets = use_context::<Wallets>().expect("No WalletContext found");

    view! {
        <select on:change=move |e| {
            let new_wallet_name = event_target_value(&e);
            logging::log!("Setting active wallet to: {}", new_wallet_name);
            set_active_wallet.set(new_wallet_name);
        }>
            {wallets.wallets.into_iter().map(|wallet| {
                view! {
                    <option value={wallet.name()}>{wallet.name()}</option>
                }
            }).collect::<Vec<_>>()}
        </select>
    }
}

#[component]
pub fn WalletApp(wallets: Vec<Box<dyn BaseWalletAdapter>>) -> impl IntoView {
    let (active_wallet, set_active_wallet) = create_signal("Phantom".to_string());
    provide_context(active_wallet);

    view! {
        <WalletProvider wallets={wallets} >
            <WalletSelect set_active_wallet=set_active_wallet />
            <WalletConnectBtn />
            <WalletView />
        </WalletProvider>
    }
}

fn main() {
    let phantom_wallet = PhantomWalletAdapter::new().unwrap();
    let solflare_wallet = SolflareWalletAdapter::new().unwrap();
    let wallets = vec![
        phantom_wallet.to_dyn_adapter(),
        solflare_wallet.to_dyn_adapter(),
    ];

    mount_to_body(|| {
        view! {
            <WalletApp wallets={wallets} />
        }
    })
}
