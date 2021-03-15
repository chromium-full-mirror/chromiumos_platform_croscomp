// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;

pub fn init(rc: &Rc<RefCell<crate::Shell>>) {
    let shell = &rc.borrow();
    let mut compositor = shell.compositor.borrow_mut();

    use super::protocol::zaura_shell::ZauraShell;
    let global = compositor.server.create_global::<ZauraShell>(12)
	.on_request(|_resource, request| {
	    use super::protocol::zaura_shell::Request;
	    match request {
		Request::GetAuraSurface { id, surface: _ } => {
		    println!("get aura surface");
		    id.on_request(|_, request| {
			println!("request for aura surface: {:?}", request)
		    });
		},
		Request::GetAuraOutput { .. } => {
		    println!("get aura output");
		},
	    }
	});
    Box::into_raw(Box::new(global));
}
