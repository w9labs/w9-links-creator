use axum::{extract::{Form,Query,State},http::StatusCode,response::{Html,IntoResponse,Redirect},routing::{get,post},Json,Router};
use axum_extra::extract::CookieJar;
use chrono::Utc;
use nanoid::nanoid;
use serde::Deserialize;
use sha2::{Digest,Sha256};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_postgres::{Client,NoTls};
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer,trace::TraceLayer,services::ServeDir};
use tracing_subscriber::{layer::SubscriberExt,util::SubscriberInitExt};
use uuid::Uuid;

const CSS:&str=include_str!("../infra/templates/voxel.css");
const W9_DB:&str="https://db.w9.nu";

#[derive(Clone)]
pub struct AppState{
    pub db:Arc<Client>,
    pub http_client:reqwest::Client,
    pub base_url:String
}

fn layout(t:&str,b:&str,n:&str)->String{
    format!(r#"<!DOCTYPE html><html><head><meta charset="UTF-8"/><meta name="viewport" content="width=device-width,initial-scale=1.0"/><title>{} — W9 Links</title><style>{}</style></head><body><div class="app"><nav class="nav"><a href="/" class="brand"><img src="/w9-logo/wordmark-light.svg" alt="W9"/><span>Links</span></a>{}</nav>{}</div></body></html>"#,t,CSS,n,b)
}
fn pub_layout(t:&str,b:&str)->String{layout(t,b,r#"<a href="/login">Login</a>"#)}
fn user_layout(t:&str,b:&str)->String{layout(t,b,r#"<a href="/links">Links</a><a href="/notes">Notes</a><a href="/logout">Logout</a>"#)}

fn set_s(j:CookieJar,t:String)->CookieJar{
    j.add(axum_extra::extract::cookie::Cookie::build(("w9_link_session",t)).path("/").http_only(true).same_site(axum_extra::extract::cookie::SameSite::Lax).max_age(time::Duration::days(7)).finish())
}
fn clr_s(j:CookieJar)->CookieJar{j.remove(axum_extra::extract::cookie::Cookie::named("w9_link_session"))}
fn get_s(j:&CookieJar)->Option<String>{j.get("w9_link_session").map(|c|c.value().to_string())}

async fn verify(a:&AppState,t:&str)->Option<serde_json::Value>{
    let r=a.http_client.get(format!("{}/api/auth/me",W9_DB)).header("Authorization",format!("Bearer {}",t)).send().await.ok()?;
    if r.status().is_success(){r.json().await.ok()}else{None}
}
async fn require(j:&CookieJar,a:&AppState)->Option<serde_json::Value>{let t=get_s(j)?;verify(a,&t).await}

fn home_html()->String{
    pub_layout("W9 Links",r#"<div class="hero"><img class="hero-logo" src="/w9-logo/hero-transparent.svg" alt="W9 Links"/><h1>🔗 W9 Links</h1><p>Short links on w9.nu / w9.se + private note drops</p><div class="flex mt-3" style="justify-content:center"><a href="/login" class="btn">Login with W9</a></div></div><div class="grid mt-3"><div class="card"><h3>📎 Short Links</h3><p class="text-sm">Create branded short URLs with click tracking.</p></div><div class="card"><h3>📝 Note Drops</h3><p class="text-sm">Password-protected notes that auto-destroy.</p></div></div>"#)
}
fn login_html()->String{
    pub_layout("Login",r#"<div class="card" style="max-width:420px;margin:3rem auto;text-align:center"><h1>🔗 W9 Links</h1><p class="text-sm text-muted mb-2">Sign in with W9 DB</p><a href="https://db.w9.nu/oauth/authorize?redirect_uri=https://links.w9.nu/oauth/callback&response_type=code&client_id=w9-links" class="btn" style="width:100%">Login with W9 DB</a></div>"#)
}

fn links_html(l:&[(String,String,i64,Option<String>)],m:Option<&str>)->String{
    let al=m.map(|x|format!(r#"<div class="alert alert--ok">{}</div>"#,x)).unwrap_or_default();
    let rows:String=l.iter().map(|(c,u,cl,e)|{
        let ex=e.as_deref().unwrap_or("Never");
        format!(r#"<tr><td><a href="/s/{c}">{c}</a></td><td class="text-sm">{u}</td><td>{cl}</td><td class="text-xs">{ex}</td></tr>"#,c=c,u=u,cl=cl,ex=ex)
    }).collect();
    user_layout("Links",&format!(r#"<div class="card" style="max-width:700px;margin:2rem auto"><h1>📎 Links</h1>{}<form method="POST" action="/links"><label>URL</label><input type="url" name="url" required placeholder="https://..."/><label>Custom code (optional)</label><input type="text" name="code" placeholder="abc123"/><button type="submit" class="btn mt-1" style="width:100%">Create</button></form><h2 class="mt-3">My Links</h2><table><tr><th>Code</th><th>Target</th><th>Clicks</th><th>Expires</th></tr>{}</table></div>"#,al,rows))
}

fn notes_html(n:&[(String,String,i32,Option<i32>,String)],m:Option<&str>)->String{
    let al=m.map(|x|format!(r#"<div class="alert alert--ok">{}</div>"#,x)).unwrap_or_default();
    let rows:String=n.iter().map(|(c,p,v,mx,e)|{
        let mv=mx.map(|x|x.to_string()).unwrap_or_else(||"∞".into());
        format!(r#"<tr><td><a href="/n/{c}">{c}</a></td><td class="text-sm">{p}</td><td>{v}/{mv}</td><td class="text-xs">{e}</td></tr>"#,c=c,p=p,v=v,mv=mv,e=e)
    }).collect();
    user_layout("Notes",&format!(r#"<div class="card" style="max-width:700px;margin:2rem auto"><h1>📝 Notes</h1>{}<form method="POST" action="/notes"><label>Content</label><textarea name="content" rows="6" required placeholder="Your secret..."></textarea><label>Password (optional)</label><input type="password" name="password" placeholder="Leave blank for public"/><button type="submit" class="btn mt-1" style="width:100%">Create Note</button></form><h2 class="mt-3">My Notes</h2><table><tr><th>Code</th><th>Preview</th><th>Views</th><th>Expires</th></tr>{}</table></div>"#,al,rows))
}

fn note_view_html(c:&str,ct:&str)->String{
    user_layout("View Note",&format!(r#"<div class="card" style="max-width:700px;margin:2rem auto"><h1>📝 Note: {}</h1><div class="code">{}</div><a href="/notes" class="btn mt-2">Back</a></div>"#,c,ct))
}

#[derive(Debug,Deserialize)]
struct LinkReq{url:String,code:Option<String>}
#[derive(Debug,Deserialize)]
struct NoteReq{content:String,password:Option<String>,ttl_hours:Option<i64>}

async fn home()->Html<String>{Html(home_html())}
async fn login_page()->Html<String>{Html(login_html())}

async fn oauth_cb(State(s):State<AppState>,jar:CookieJar,Query(q):Query<serde_json::Value>)->impl IntoResponse{
    let code=match q.get("code").and_then(|v|v.as_str()){Some(c)=>c.to_string(),None=>return Html(login_html()).into_response()};
    let res=match s.http_client.post(format!("{}/oauth/token",W9_DB)).form(&[("grant_type","authorization_code"),("code",&code),("redirect_uri","https://links.w9.nu/oauth/callback")]).send().await{Ok(r)=>r,Err(_)=>return Html(login_html()).into_response()};
    let json=match res.json::<serde_json::Value>().await{Ok(j)=>j,Err(_)=>return Html(login_html()).into_response()};
    let token=match json.get("access_token").and_then(|v|v.as_str()){Some(t)=>t.to_string(),None=>return Html(login_html()).into_response()};
    (set_s(jar,token),Redirect::to("/links")).into_response()
}
async fn logout(jar:CookieJar)->impl IntoResponse{(clr_s(jar),Redirect::to("/")).into_response()}

async fn links_page(State(s):State<AppState>,jar:CookieJar)->impl IntoResponse{
    if require(&jar,&s).await.is_none(){return Redirect::to("/login").into_response();}
    let l=match s.db.query("SELECT code,target_url,clicks,expires_at::text FROM short_links ORDER BY created_at DESC LIMIT 50",&[]).await{
        Ok(r)=>r.iter().map(|x|(x.get(0),x.get(1),x.get(2),x.get(3))).collect(),Err(_)=>Vec::new()};
    Html(links_html(&l,None)).into_response()
}

async fn links_post(State(s):State<AppState>,jar:CookieJar,Form(f):Form<LinkReq>)->impl IntoResponse{
    if require(&jar,&s).await.is_none(){return Redirect::to("/login").into_response();}
    let code=f.code.filter(|c|!c.is_empty()).unwrap_or_else(||nanoid!(8));
    let id=Uuid::new_v4();
    match s.db.execute("INSERT INTO short_links(id,code,target_url,domain,title,expires_at)VALUES($1,$2,$3,$4,$5,$6)",&[&id,&code,&f.url,&"w9.nu",&Option::<String>::None,&Option::<chrono::DateTime<Utc>>::None]).await{
        Ok(_)=>{
            let l=match s.db.query("SELECT code,target_url,clicks,expires_at::text FROM short_links ORDER BY created_at DESC LIMIT 50",&[]).await{
                Ok(r)=>r.iter().map(|x|(x.get(0),x.get(1),x.get(2),x.get(3))).collect(),Err(_)=>Vec::new()};
            Html(links_html(&l,Some(&format!("Link created: w9.nu/s/{}",code)))).into_response()
        },
        Err(e)=>Html(links_html(&[],Some(&format!("Error: {}",e)))).into_response()
    }
}

async fn notes_page(State(s):State<AppState>,jar:CookieJar)->impl IntoResponse{
    if require(&jar,&s).await.is_none(){return Redirect::to("/login").into_response();}
    let n=match s.db.query("SELECT code,LEFT(content,50) as preview,views,max_views,expires_at::text FROM notes ORDER BY created_at DESC LIMIT 50",&[]).await{
        Ok(r)=>r.iter().map(|x|(x.get(0),x.get(1),x.get(2),x.get(3),x.get(4))).collect(),Err(_)=>Vec::new()};
    Html(notes_html(&n,None)).into_response()
}

async fn notes_post(State(s):State<AppState>,jar:CookieJar,Form(f):Form<NoteReq>)->impl IntoResponse{
    if require(&jar,&s).await.is_none(){return Redirect::to("/login").into_response();}
    let code=nanoid!(8);
    let exp=chrono::Utc::now()+chrono::Duration::hours(f.ttl_hours.unwrap_or(24));
    let pw=f.password.as_ref().map(|p|{let mut h=Sha256::new();h.update(p.as_bytes());format!("{:x}",h.finalize())});
    let id=Uuid::new_v4();
    match s.db.execute("INSERT INTO notes(id,code,content,password_hash,expires_at,max_views)VALUES($1,$2,$3,$4,$5,$6)",&[&id,&code,&f.content,&pw,&exp,&Option::<i32>::None]).await{
        Ok(_)=>Html(notes_html(&[],Some(&format!("Note created: w9.nu/n/{}",code)))).into_response(),
        Err(e)=>Html(notes_html(&[],Some(&format!("Error: {}",e)))).into_response()
    }
}

async fn view_note(State(s):State<AppState>,axum::extract::Path(code):axum::extract::Path<String>)->impl IntoResponse{
    let row=match s.db.query_opt("SELECT content,password_hash,views,max_views FROM notes WHERE code=$1 AND expires_at>$2",&[&code,&Utc::now()]).await{
        Ok(Some(r))=>r,
        _=>return Html(layout("Not Found",r#"<div class="card" style="max-width:400px;margin:3rem auto;text-align:center"><h1>404</h1><p>Note not found.</p></div>"#,"")).into_response()
    };
    let content:String=row.get("content");
    let pw:Option<String>=row.get("password_hash");
    let views:i32=row.get("views");
    let mx:Option<i32>=row.get("max_views");
    if let Some(m)=mx{
        if views>=m{
            let _=s.db.execute("DELETE FROM notes WHERE code=$1",&[&code]).await;
            return Html(layout("Gone",r#"<div class="card" style="max-width:400px;margin:3rem auto;text-align:center"><h1>🗑️ Consumed</h1><p>Note destroyed.</p></div>"#,"")).into_response()
        }
    }
    let _=s.db.execute("UPDATE notes SET views=views+1 WHERE code=$1",&[&code]).await;
    Html(note_view_html(&code,&content)).into_response()
}

async fn health(State(s):State<AppState>)->impl IntoResponse{
    match s.db.query_one("SELECT 1",&[]).await{
        Ok(_)=>(StatusCode::OK,Json(serde_json::json!({"status":"ok","service":"w9-links-creator","database":"connected","timestamp":Utc::now().to_rfc3339()}))),
        Err(e)=>(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"status":"error","error":e.to_string()})))
    }
}

#[tokio::main]
async fn main()->anyhow::Result<()>{
    tracing_subscriber::registry().with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_|"info".into())).with(tracing_subscriber::fmt::layer()).init();
    dotenvy::dotenv().ok();
    let port=std::env::var("PORT").unwrap_or_else(|_|"8085".into());
    let db_url=std::env::var("W9_LINKS_DB_URL").or_else(|_|std::env::var("DATABASE_URL")).unwrap_or_else(|_|"postgres://w9_admin:password@w9-postgres:5432/w9_links".into());
    let base=std::env::var("BASE_URL").unwrap_or_else(|_|"https://w9.nu".into());
    let(client,conn)=tokio_postgres::connect(&db_url,NoTls).await?;
    tokio::spawn(async move{if let Err(e)=conn.await{tracing::error!("DB:{}",e);}});
    client.query_one("SELECT 1",&[]).await?;
    let state=AppState{db:Arc::new(client),http_client:reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build()?,base_url:base};
    let router=Router::new()
        .nest_service("/w9-logo", ServeDir::new("public/w9-logo"))
        .route("/",get(home))
        .route("/login",get(login_page))
        .route("/oauth/callback",get(oauth_cb))
        .route("/logout",get(logout))
        .route("/links",get(links_page))
        .route("/links",post(links_post))
        .route("/notes",get(notes_page))
        .route("/notes",post(notes_post))
        .route("/n/:code",get(view_note))
        .route("/api/health",get(health))
        .with_state(state)
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()).layer(CorsLayer::permissive()));
    let addr=format!("0.0.0.0:{}",port);
    let listener=TcpListener::bind(&addr).await?;
    tracing::info!("W9 Links on {}",addr);
    axum::serve(listener,router).await?;
    Ok(())
}
