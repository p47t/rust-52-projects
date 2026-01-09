use lambda_runtime::{service_fn, Error, LambdaEvent};

mod alexa;

#[tokio::main]
async fn main() -> Result<(), Error> {
    simple_logger::init_with_level(log::Level::Info)
        .map_err(|e| Error::from(Box::new(e) as Box<dyn std::error::Error + Send + Sync>))?;

    lambda_runtime::run(service_fn(my_handler)).await
}

fn build_quote_response(quote: &str, author: &str) -> alexa::ResponseRoot {
    alexa::ResponseRoot {
        version: "1.0".to_string(),
        session_attributes: None,
        response: alexa::Response {
            output_speech: Some(alexa::OutputSpeech {
                r#type: "PlainText".to_string(),
                text: Some(format!("{author} said {quote}")),
                ssml: None,
                play_behavior: None,
            }),
            card: None,
            reprompt: None,
            should_end_session: None,
            directives: None,
        },
    }
}

async fn my_handler(event: LambdaEvent<alexa::RequestRoot>) -> Result<alexa::ResponseRoot, Error> {
    let (_event, _context) = event.into_parts();

    Ok(build_quote_response(
        "The best way to predict the future is to invent it.",
        "Alan Kay",
    ))
}
