use leptos::{ev::MouseEvent, logging::{error, log}, prelude::*, server_fn::{codec::JsonEncoding, BoxedStream, Websocket}, task::spawn_local};
use leptos_meta::*;
use leptos_router::{components::{Route, Router, Routes}, *};
use futures::{channel::mpsc::{self, Receiver, Sender}, future::{select, Either}, SinkExt, StreamExt};
use futures::channel::oneshot;

#[server(protocol = Websocket<JsonEncoding, JsonEncoding>)]
async fn some_websocket_thing(
    input: BoxedStream<String, ServerFnError>, // we don't do anything with this rn
) -> Result<BoxedStream<String, ServerFnError>, ServerFnError> {
    let mut input = input;

    let (mut tx, rx) = mpsc::channel(1);

    tokio::spawn(async move {
        while let Some(messages) = input.next().await {
            match messages {
                Ok(_) => {
                    let _ = tx.send(Ok("World!".to_string())).await;
                },
                _ => {
                    error!("some error");
                }
            }

        }
    });

    Ok(rx.into())
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/websocket_not_closing_minimal_repro.css"/>

        // sets the document title
        <Title text="Welcome to Leptos"/>

        // content for this welcome page
        <Router>
            <main>
                <Routes fallback=|| "not found">
                    <Route path=StaticSegment("other") view=Other/>
                    <Route path=StaticSegment("app") view=HomePage/>
                </Routes>
            </main>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    // Creates a reactive value to update the button
    view! {
        <p><a href="/other">"Click here"</a>" to go to another page that will open a web socket. Then come back here and see the web socket will still be open"</p>
    }
}

#[component]
fn Other() -> impl IntoView {
    let (received, set_received)= signal(String::new());

    let (tx, rx) = mpsc::channel(1);
    if cfg!(feature = "hydrate") {
        let (cancel_send, cancel) = oneshot::channel::<()>();
        spawn_local(async move {
            let mut cancel_future = cancel;
            
            match some_websocket_thing(rx.into()).await {
                Ok(mut messages) => {
                    let mut message_stream = messages.next();

                    loop {
                        match select(cancel_future, message_stream).await {
                            Either::Left((_, _)) => {
                                log!("WebSocket task cancelled because component was unmounted");
                                break;
                            }
                            Either::Right((msg_opt, next_cancel_future)) => {
                                cancel_future = next_cancel_future;
                                if let Some(Ok(message)) = msg_opt {
                                    set_received.update(|received| {
                                        *received = format!("{} {}", received, message);
                                    })
                                }

                                message_stream = messages.next();
                            }
                        }
                    }
                },
                Err(e) => error!("Some error happened {}", e)
            }

            log!("spawned task ending...");
        });

        on_cleanup(move || {
            // Send the cancellation signal to shut down the spawned task
            let _ = cancel_send.send(());
            log!("Sent cancellation signal to WebSocket task");
        });
    }

    let handler = move |_: MouseEvent| {
        let mut tx_clone: Sender<Result<String, _>> = tx.clone();
        spawn_local(async move {
            match tx_clone.send(Ok("Hello".to_string())).await {
                Ok(_) => log!("message sent"),
                Err(e) => error!("error sending {}", e)
            }
        });
    };
    view! {
        <p><a href="/app">"Go back here"</a>" and note the websocket is still open"</p>
        <button on:click=handler>"Send and receive"</button>
        {received}
    }
}
