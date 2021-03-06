// Copyright 2016 Google Inc. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Module to run a plugin.

use std::io::BufReader;
use std::env;
use std::path::PathBuf;
use std::process::{Command,Stdio,ChildStdin};
use std::sync::{Arc, Mutex};
use std::thread;
use serde_json::Value;

use rpc_peer::{RpcPeer,RpcWriter};
use editor::Editor;

pub type PluginPeer = RpcWriter<ChildStdin>;

pub fn start_plugin(editor: Arc<Mutex<Editor>>) {
    thread::spawn(move || {
        let mut pathbuf: PathBuf = match env::current_exe() {
            Ok(pathbuf) => pathbuf,
            Err(e) => {
                print_err!("Could not get current path: {}", e);
                return;
            }
        };
        pathbuf.pop();
        pathbuf.push("python");
        pathbuf.push("plugin.py");
        //print_err!("path = {:?}", pathbuf);
        let mut child = Command::new(&pathbuf)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("plugin failed to start");
        let child_stdin = child.stdin.take().unwrap();
        let child_stdout = child.stdout.take().unwrap();
        let mut peer = RpcPeer::new(BufReader::new(child_stdout), child_stdin);
        let peer_w = peer.get_writer();
        peer_w.send_rpc_async("ping", &Value::Null);
        editor.lock().unwrap().on_plugin_connect(&peer_w);
        peer.mainloop(|method, params| rpc_handler(&editor, method, params));
        let status = child.wait();
        print_err!("child exit = {:?}", status);
    });
}

fn rpc_handler(editor: &Arc<Mutex<Editor>>, method: &str, params: &Value) -> Option<Value> {
    let mut editor = editor.lock().unwrap();
    match method {
        // TODO: parse json into enum first, just like front-end RPC
        // (this will also improve error handling, no panic on malformed request from plugin)
        "n_lines" => Some(Value::U64(editor.plugin_n_lines() as u64)),
        "get_line" => {
            let line = params.as_object().and_then(|dict| dict.get("line").and_then(Value::as_u64)).unwrap();
            let result = editor.plugin_get_line(line as usize);
            Some(Value::String(result))
        }
        "set_line_fg_spans" => {
            let dict = params.as_object().unwrap();
            let line_num = dict.get("line").and_then(Value::as_u64).unwrap() as usize;
            let spans = dict.get("spans").unwrap();
            editor.plugin_set_line_fg_spans(line_num, spans);
            None
        }
        _ => {
            print_err!("unknown plugin callback method: {}", method);
            None
        }
    }
}
