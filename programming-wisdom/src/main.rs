use lambda_runtime::{error::HandlerError, lambda, Context};
use std::error::Error;

mod alexa;

fn main() -> Result<(), Box<dyn Error>> {
    lambda!(my_handler);
    Ok(())
}

fn build_quote_response(quote: &str, author: &str) -> alexa::ResponseRoot {
    alexa::ResponseRoot {
        version: "1.0".to_string(),
        session_attributes: None,
        response: alexa::Response {
            output_speech: Some(alexa::OutputSpeech {
                r#type: "PlainText".to_string(),
                text: Some(author.to_string() + " said " + quote),
                ssml: None,
                play_behavior: None,
            }),
            card: None,
            reprompt: None,
            should_end_session: None,
            directives: None,
        }
    }
}

fn my_handler(_e: alexa::RequestRoot, _ctx: Context) -> Result<alexa::ResponseRoot, HandlerError> {
    Ok(build_quote_response(
        "The best way to predict the future is to invent it.",
        "Alan Kay"))
}