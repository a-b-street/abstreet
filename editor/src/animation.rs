// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

#[derive(PartialEq)]
pub enum EventLoopMode {
    Animation,
    InputOnly,
}

impl EventLoopMode {
    pub fn merge(self, other: EventLoopMode) -> EventLoopMode {
        match self {
            EventLoopMode::Animation => EventLoopMode::Animation,
            _ => other,
        }
    }
}
