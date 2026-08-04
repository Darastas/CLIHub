use egui::Response; pub fn test(resp: Response) { let _ = resp.dnd_release_payload::<usize>(); }
