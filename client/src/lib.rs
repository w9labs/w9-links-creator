use leptos::*;
use leptos_meta::*;
use leptos_router::*;

mod components;
mod pages;
use components::{Header, Footer};

use pages::HomePage;
use pages::CreatePage;
use pages::LinksPage;
use pages::LoginPage;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Title text="W9 Links"/>
        <Meta name="viewport" content="width=device-width, initial-scale=1"/>
        <Stylesheet id="voxel" href="/pkg/w9-links-creator-client.css"/>
        <Router>
            <div class="app-container">
                <Header/>
                <main class="main-content">
                    <Routes>
                        <Route path="home" view=HomePage/>
                        <Route path="create" view=CreatePage/>
                        <Route path="links" view=LinksPage/>
                        <Route path="login" view=LoginPage/>
                    </Routes>
                </main>
                <Footer/>
            </div>
        </Router>
    }
}
