use terminal_protocol::{
    C0Control, CsiParam, CsiSequence, DcsCommand, EscapeSequence, M1_PAYLOAD_LIMIT_BYTES,
    OscCommand, Payload, Printable, StringControl, StringControlKind, TerminalAction,
};

pub(crate) trait ActionSink {
    fn push_action(&mut self, action: TerminalAction);
}

pub(crate) struct VteAdapter {
    parser: vte::Parser<M1_PAYLOAD_LIMIT_BYTES>,
    pending_escape: bool,
    string_control: Option<StringControlBuilder>,
    dcs: Option<DcsBuilder>,
    suppress_next_st_terminator: bool,
}

impl VteAdapter {
    pub(crate) fn new() -> Self {
        Self {
            parser: vte::Parser::<M1_PAYLOAD_LIMIT_BYTES>::default(),
            pending_escape: false,
            string_control: None,
            dcs: None,
            suppress_next_st_terminator: false,
        }
    }

    pub(crate) fn advance(&mut self, bytes: &[u8], sink: &mut impl ActionSink) {
        if self.pending_escape && bytes.is_empty() {
            return;
        }

        let mut recorder =
            ActionRecorder::new(sink, self.dcs.take(), self.suppress_next_st_terminator);

        if self.pending_escape {
            self.pending_escape = false;
            if recorder.is_inside_dcs() {
                self.parser.advance(&mut recorder, &[0x1b, bytes[0]]);
                self.advance_without_pending_escape(&bytes[1..], &mut recorder);
            } else if let Some(target) = string_control_target(bytes[0]) {
                self.string_control = Some(StringControlBuilder::new(target));
                self.advance_without_pending_escape(&bytes[1..], &mut recorder);
            } else {
                self.parser.advance(&mut recorder, &[0x1b, bytes[0]]);
                self.advance_without_pending_escape(&bytes[1..], &mut recorder);
            }
        } else {
            self.advance_without_pending_escape(bytes, &mut recorder);
        }

        self.dcs = recorder.dcs;
        self.suppress_next_st_terminator = recorder.suppress_next_st_terminator;
    }

    fn advance_without_pending_escape<S: ActionSink>(
        &mut self,
        bytes: &[u8],
        recorder: &mut ActionRecorder<'_, S>,
    ) {
        let mut chunk_start = 0usize;
        let mut index = 0usize;

        while index < bytes.len() {
            if let Some(builder) = &mut self.string_control {
                let (consumed, control) = builder.consume(&bytes[index..]);
                index += consumed;
                chunk_start = index;

                if let Some(action) = control {
                    self.string_control = None;
                    recorder.push_action(action);
                }

                continue;
            }

            if bytes[index] == 0x1b {
                if chunk_start < index {
                    self.parser.advance(recorder, &bytes[chunk_start..index]);
                    chunk_start = index;
                }

                if index + 1 == bytes.len() {
                    self.pending_escape = true;
                    return;
                }

                if !recorder.is_inside_dcs()
                    && let Some(target) = string_control_target(bytes[index + 1])
                {
                    self.string_control = Some(StringControlBuilder::new(target));
                    index += 2;
                    chunk_start = index;
                    continue;
                }
            }

            index += 1;
        }

        if self.string_control.is_none() && chunk_start < bytes.len() {
            self.parser.advance(recorder, &bytes[chunk_start..]);
        }
    }
}

struct ActionRecorder<'a, S: ActionSink> {
    sink: &'a mut S,
    dcs: Option<DcsBuilder>,
    suppress_next_st_terminator: bool,
}

impl<'a, S: ActionSink> ActionRecorder<'a, S> {
    fn new(sink: &'a mut S, dcs: Option<DcsBuilder>, suppress_next_st_terminator: bool) -> Self {
        Self {
            sink,
            dcs,
            suppress_next_st_terminator,
        }
    }

    fn push_action(&mut self, action: TerminalAction) {
        self.sink.push_action(action);
    }

    fn is_inside_dcs(&self) -> bool {
        self.dcs.is_some()
    }
}

impl<S: ActionSink> vte::Perform for ActionRecorder<'_, S> {
    fn print(&mut self, ch: char) {
        self.push_action(TerminalAction::Print(Printable::new(ch)));
    }

    fn execute(&mut self, byte: u8) {
        self.push_action(TerminalAction::Control(C0Control::from_byte(byte)));
    }

    fn hook(&mut self, params: &vte::Params, intermediates: &[u8], ignored: bool, action: char) {
        self.dcs = Some(DcsBuilder::new(
            csi_params_from_vte(params),
            intermediates,
            ignored,
            action,
        ));
    }

    fn put(&mut self, byte: u8) {
        if let Some(dcs) = &mut self.dcs {
            dcs.push(byte);
        }
    }

    fn unhook(&mut self) {
        if let Some(dcs) = self.dcs.take() {
            self.push_action(TerminalAction::Dcs(dcs.finish()));
            self.suppress_next_st_terminator = true;
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
        self.push_action(TerminalAction::Osc(OscCommand::from_parts(
            params.iter().copied(),
            bell_terminated,
            M1_PAYLOAD_LIMIT_BYTES,
        )));
        if !bell_terminated {
            self.suppress_next_st_terminator = true;
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        ignored: bool,
        action: char,
    ) {
        self.push_action(TerminalAction::Csi(CsiSequence::new(
            csi_params_from_vte(params),
            intermediates,
            ignored,
            action,
        )));
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], ignored: bool, byte: u8) {
        if self.suppress_next_st_terminator && intermediates.is_empty() && !ignored && byte == b'\\'
        {
            self.suppress_next_st_terminator = false;
            return;
        }

        self.suppress_next_st_terminator = false;
        self.push_action(TerminalAction::Escape(EscapeSequence::new(
            intermediates,
            ignored,
            byte,
        )));
    }
}

struct StringControlBuilder {
    target: StringControlTarget,
    bytes: Vec<u8>,
    original_len: usize,
    pending_escape: bool,
}

impl StringControlBuilder {
    fn new(target: StringControlTarget) -> Self {
        Self {
            target,
            bytes: Vec::new(),
            original_len: 0,
            pending_escape: false,
        }
    }

    fn consume(&mut self, bytes: &[u8]) -> (usize, Option<TerminalAction>) {
        for (offset, byte) in bytes.iter().copied().enumerate() {
            if self.pending_escape {
                self.pending_escape = false;
                if byte == b'\\' {
                    return (
                        offset + 1,
                        Some(self.finish(StringTerminator::StringTerminator)),
                    );
                }

                self.push_payload_byte(0x1b);
                self.push_payload_byte(byte);
                continue;
            }

            if self.target == StringControlTarget::Osc && byte == 0x07 {
                return (offset + 1, Some(self.finish(StringTerminator::Bell)));
            }

            if byte == 0x1b {
                self.pending_escape = true;
                continue;
            }

            self.push_payload_byte(byte);
        }

        (bytes.len(), None)
    }

    fn push_payload_byte(&mut self, byte: u8) {
        self.original_len += 1;
        if self.bytes.len() < M1_PAYLOAD_LIMIT_BYTES {
            self.bytes.push(byte);
        }
    }

    fn finish(&self, terminator: StringTerminator) -> TerminalAction {
        let payload = Payload::from_limited_bytes(
            self.bytes.clone(),
            self.original_len,
            M1_PAYLOAD_LIMIT_BYTES,
        );

        match self.target {
            StringControlTarget::Osc => TerminalAction::Osc(OscCommand::from_payload(
                payload,
                terminator == StringTerminator::Bell,
            )),
            StringControlTarget::Apc => TerminalAction::Apc(StringControl::new(
                StringControlKind::ApplicationProgramCommand,
                payload,
            )),
            StringControlTarget::Pm => TerminalAction::Pm(StringControl::new(
                StringControlKind::PrivacyMessage,
                payload,
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StringControlTarget {
    Osc,
    Apc,
    Pm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StringTerminator {
    Bell,
    StringTerminator,
}

struct DcsBuilder {
    params: Vec<CsiParam>,
    intermediates: Vec<u8>,
    ignored: bool,
    action: char,
    bytes: Vec<u8>,
    original_len: usize,
}

impl DcsBuilder {
    fn new(params: Vec<CsiParam>, intermediates: &[u8], ignored: bool, action: char) -> Self {
        Self {
            params,
            intermediates: intermediates.to_vec(),
            ignored,
            action,
            bytes: Vec::new(),
            original_len: 0,
        }
    }

    fn push(&mut self, byte: u8) {
        self.original_len += 1;
        if self.bytes.len() < M1_PAYLOAD_LIMIT_BYTES {
            self.bytes.push(byte);
        }
    }

    fn finish(self) -> DcsCommand {
        DcsCommand::new(
            self.params,
            &self.intermediates,
            self.ignored,
            self.action,
            Payload::from_limited_bytes(self.bytes, self.original_len, M1_PAYLOAD_LIMIT_BYTES),
        )
    }
}

fn csi_params_from_vte(params: &vte::Params) -> Vec<CsiParam> {
    params
        .iter()
        .map(|param| CsiParam::new(param.to_vec()))
        .collect()
}

fn string_control_target(byte: u8) -> Option<StringControlTarget> {
    match byte {
        b']' => Some(StringControlTarget::Osc),
        b'_' => Some(StringControlTarget::Apc),
        b'^' => Some(StringControlTarget::Pm),
        _ => None,
    }
}
