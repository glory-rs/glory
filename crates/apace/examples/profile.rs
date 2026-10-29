//! Headless native document + CPU pixels, not a window/FPS benchmark.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use glory_apace::{Raster, Renderer};
    use glory_render::profile::{Profile, Snapshot};
    use glory_render::style::{Length, Style};
    use glory_ui::{App, list, text, view};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::{Duration, Instant};
    let mut arguments = std::env::args().skip(1);
    let (mut quick, mut enabled, mut measure_limit) = (false, true, 1usize);
    let mut scrolling = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--quick" => quick = true,
            "--no-profile" => enabled = false,
            "--scroll" => scrolling = true,
            "--measure-limit" => {
                measure_limit = arguments.next().ok_or("missing measure limit")?.parse()?;
                if measure_limit == 0 {
                    return Err("measure limit must be positive".into());
                }
            }
            _ => return Err("use --quick, --no-profile, --scroll and/or --measure-limit N".into()),
        }
    }
    let frames = if quick { 12usize } else { 100 };
    let warm = if quick { 3usize } else { 10 };
    fn phases(
        rows: usize,
        measured: bool,
        repetition: usize,
        pipeline: &str,
        interval: &str,
        snapshot: Snapshot,
    ) {
        for (phase, s) in snapshot.iter().filter(|(_, s)| s.calls > 0) {
            println!(
                "{{\"type\":\"phase\",\"rows\":{rows},\"measured\":{measured},\"repetition\":{repetition},\"pipeline\":\"{pipeline}\",\"interval\":\"{interval}\",\"phase\":\"{}\",\"calls\":{},\"units\":{},\"inclusive_ns\":{},\"max_ns\":{},\"invalid_clock\":{}}}",
                phase.name(),
                s.calls,
                s.units,
                s.elapsed.as_nanos(),
                s.max.as_nanos(),
                s.invalid_clock
            );
        }
    }
    for rows in [1_000_000, 100_000_000] {
        for measured in [false, true] {
            for repetition in 0..if quick { 1 } else { 3 } {
                for progressive in if repetition % 2 == 0 {
                    [true, false]
                } else {
                    [false, true]
                } {
                    let pipeline = if progressive {
                        "progressive"
                    } else {
                        "settled"
                    };
                    let setup = Instant::now();
                    let mut renderer = None;
                    let app = App::new(|id| {
                        let value = Renderer::new(id);
                        renderer = Some(value.clone());
                        value
                    });
                    let renderer = renderer.unwrap();
                    renderer.set_viewport(160, 120, 1.)?;
                    let setup_ns = setup.elapsed().as_nanos();
                    let profile = Profile::native();
                    app.set_profile(enabled.then(|| profile.clone()));
                    let position = app.scope().cage(0.);
                    let reads = Rc::new(Cell::new(0usize));
                    let keys = reads.clone();
                    let list_state = Rc::new(RefCell::new(None));
                    let observed = list_state.clone();
                    let widget = list(
                        rows,
                        move |i| {
                            keys.set(keys.get() + 1);
                            i
                        },
                        move |_, key| {
                            view()
                                .style(Style::new().height(Length::Px(if measured {
                                    [24., 28., 32.][key % 3]
                                } else {
                                    24.
                                })))
                                .fill(text().content(format!("row:{key}")))
                        },
                    )
                    .height(120.)
                    .overscan(2)
                    .row_limit(20)
                    .inspect(move |stats| *observed.borrow_mut() = Some(stats))
                    .position(position);
                    let mut raster = Raster::new();
                    let settle = |raster: &mut Raster| -> Result<(), Box<dyn std::error::Error>> {
                        if !progressive {
                            let turn = app.settle(App::DEFAULT_SETTLE_PASSES)?;
                            if turn.pending {
                                return Err("native observation budget exhausted".into());
                            }
                            raster.draw(&renderer)?;
                            return Ok(());
                        }
                        for _ in 0..16 {
                            app.pump()?;
                            if !raster.draw(&renderer)?.rendered {
                                return Ok(());
                            }
                        }
                        Err("native profile did not settle".into())
                    };
                    let cold = Instant::now();
                    app.mount(if measured {
                        widget.measure(24.).measure_limit(measure_limit)
                    } else {
                        widget.row_height(24.)
                    })?;
                    settle(&mut raster)?;
                    // CSS height does not size an inline Text. Verify actual
                    // mounted row boxes, not just the declarations we wrote.
                    let row_nodes: Vec<_> = {
                        let tree = app.tree();
                        let root = tree.node(tree.roots()[0])?;
                        tree.node(root.children[0])?
                            .children
                            .iter()
                            .map(|id| {
                                (
                                    *id,
                                    tree.node(*id)
                                        .unwrap()
                                        .descriptor
                                        .view()
                                        .item
                                        .unwrap()
                                        .index,
                                )
                            })
                            .collect()
                    };
                    let verified = Rc::new(Cell::new(0usize));
                    for &(id, index) in &row_nodes {
                        let expected = if measured {
                            [24., 28., 32.][index % 3]
                        } else {
                            24.
                        };
                        let verified = verified.clone();
                        app.request(glory_render::Query::Bounds(id), move |result| {
                            let glory_render::QueryValue::Bounds(bounds) = result.unwrap() else {
                                panic!("row bounds expected")
                            };
                            assert!(
                                (bounds.height - expected).abs() < 0.01,
                                "row {index}: {bounds:?} != {expected}"
                            );
                            verified.set(verified.get() + 1);
                        })?;
                    }
                    settle(&mut raster)?;
                    assert_eq!(verified.get(), row_nodes.len());
                    let cold_ns = cold.elapsed().as_nanos();
                    phases(
                        rows,
                        measured,
                        repetition,
                        pipeline,
                        "cold",
                        profile.snapshot(),
                    );
                    let root = app.tree().roots()[0];
                    let engine_root = renderer.engine_id(root, glory_html::Part::Main)?;
                    // Fill the actual retained cache before timing. Even the
                    // 1024-entry case must cross its eviction boundary; 100
                    // nearby scrolls alone would never exercise that budget.
                    let precondition = Instant::now();
                    let mut precondition_steps = 0usize;
                    if measured {
                        // Keep this band disjoint from both timed workloads;
                        // filling 1024 entries must not premeasure the seeks.
                        let precondition_y = |step: usize| {
                            (rows - rows / 8 + (step * 32) % (rows / 8 - 100)) as f64 * 24.
                        };
                        loop {
                            precondition_steps += 1;
                            assert!(precondition_steps <= measure_limit + 2);
                            position.revise(|value| {
                                *value = precondition_y(precondition_steps);
                            });
                            settle(&mut raster)?;
                            if list_state.borrow().as_ref().unwrap().measured >= measure_limit {
                                // The next distant window evicts retained rows.
                                position.revise(|value| {
                                    *value = precondition_y(precondition_steps + 1);
                                });
                                settle(&mut raster)?;
                                break;
                            }
                        }
                    }
                    let precondition_ns = precondition.elapsed().as_nanos();
                    if scrolling {
                        position.revise(|value| *value = rows as f64 * 12.);
                        settle(&mut raster)?;
                    }
                    let advance = |step: usize| -> Result<(), Box<dyn std::error::Error>> {
                        if scrolling {
                            let y = renderer.inspect(|doc| {
                                doc.get_node(engine_root).unwrap().scroll_offset().y + 17.
                            });
                            app.scope().action(glory_render::Action::ScrollTo {
                                node: root,
                                x: 0.,
                                y,
                            })?;
                        } else {
                            position.revise(|v| *v = ((step * 7919) % (rows - 100)) as f64 * 24.);
                        }
                        Ok(())
                    };
                    for step in 1..=warm {
                        advance(step)?;
                        settle(&mut raster)?;
                    }
                    assert!(profile.reset());
                    let initial_reads = reads.get();
                    let mut frame_time = Duration::ZERO;
                    let first_layout = renderer.stats().layouts;
                    let mut pixel_hash = 0xcbf29ce484222325_u64;
                    let mut position_hash = 0xcbf29ce484222325_u64;
                    let mut geometry_hash = 0xcbf29ce484222325_u64;
                    let (mut final_physical, mut final_logical) = (0., 0.);
                    let mut max_measured = 0;
                    let (mut max_nodes, mut max_graph, mut max_engine) = (0, 0, 0);
                    for step in warm + 1..=warm + frames {
                        let frame = Instant::now();
                        advance(step)?;
                        settle(&mut raster)?;
                        frame_time += frame.elapsed();
                        // Verify the whole final image sequence outside the
                        // measured interval, with identical work in both builds.
                        for &byte in raster.pixels() {
                            pixel_hash = (pixel_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
                        }
                        let stats = list_state.borrow();
                        let stats = stats.as_ref().unwrap();
                        if measured {
                            assert!(stats.range.end < rows - rows / 8 - 1000);
                        }
                        max_measured = max_measured.max(stats.measured);
                        assert!(stats.measured <= measure_limit.max(stats.range.len()));
                        let physical = renderer
                            .inspect(|doc| doc.get_node(engine_root).unwrap().scroll_offset().y);
                        (final_physical, final_logical) = (physical, stats.logical);
                        for word in [
                            physical.to_bits(),
                            stats.logical.to_bits(),
                            stats.total.to_bits(),
                            stats.range.start as u64,
                            stats.range.end as u64,
                            stats.measured as u64,
                        ] {
                            position_hash = (position_hash ^ word).wrapping_mul(0x100000001b3);
                        }
                        // Validate every final mounted row's real box, including
                        // rows reached after eviction. No source-data scan.
                        let tree = app.tree();
                        for &id in &tree.node(tree.node(root)?.children[0])?.children {
                            let index = tree.node(id)?.descriptor.view().item.unwrap().index;
                            let engine = renderer.engine_id(id, glory_html::Part::Main)?;
                            let bounds = renderer
                                .inspect(|doc| doc.get_client_bounding_rect(engine).unwrap());
                            let expected = if measured {
                                [24., 28., 32.][index % 3]
                            } else {
                                24.
                            };
                            assert!((bounds.height - expected).abs() < 0.01);
                            for word in [
                                index as u64,
                                bounds.x.to_bits(),
                                bounds.y.to_bits(),
                                bounds.width.to_bits(),
                                bounds.height.to_bits(),
                            ] {
                                geometry_hash = (geometry_hash ^ word).wrapping_mul(0x100000001b3);
                            }
                        }
                        max_nodes = max_nodes.max(app.stats().nodes);
                        max_graph = max_graph.max(app.runtime_stats().node_slots);
                        max_engine = max_engine.max(renderer.stats().engine_nodes);
                    }
                    let keys = reads.get() - initial_reads;
                    let layout_passes = renderer.stats().layouts - first_layout;
                    let motion = if scrolling { "scroll" } else { "seek" };
                    assert!(keys < 10000 && max_nodes < 64 && max_graph < 256 && max_engine < 80);
                    println!(
                        "{{\"type\":\"run\",\"motion\":\"{motion}\",\"rows\":{rows},\"measured\":{measured},\"measure_limit\":{measure_limit},\"repetition\":{repetition},\"pipeline\":\"{pipeline}\",\"profile\":{enabled},\"frames\":{frames},\"setup_ns\":{setup_ns},\"cold_ns\":{cold_ns},\"precondition_ns\":{precondition_ns},\"precondition_steps\":{precondition_steps},\"max_measured\":{max_measured},\"frame_ns\":{},\"key_reads\":{keys},\"max_nodes\":{max_nodes},\"max_graph\":{max_graph},\"max_engine\":{max_engine},\"layout_passes\":{layout_passes},\"pixel_hash\":\"{pixel_hash:016x}\",\"position_hash\":\"{position_hash:016x}\",\"geometry_hash\":\"{geometry_hash:016x}\",\"final_physical\":{final_physical},\"final_logical\":{final_logical},\"verified_heights\":true,\"buffer_bytes\":{}}}",
                        frame_time.as_nanos(),
                        raster.pixels().len()
                    );
                    let draws = profile
                        .snapshot()
                        .sample(glory_render::profile::Phase::Draw)
                        .calls;
                    if !enabled {
                        assert_eq!(draws, 0);
                    } else if progressive && (measured || scrolling) {
                        assert!((frames as u64..=frames as u64 * 16).contains(&draws));
                    } else {
                        assert_eq!(draws, frames as u64);
                    }
                    phases(
                        rows,
                        measured,
                        repetition,
                        pipeline,
                        "steady",
                        profile.snapshot(),
                    );
                    assert!(profile.reset());
                    for _ in 0..5 {
                        app.pump()?;
                        assert!(!raster.draw(&renderer)?.rendered);
                    }
                    for phase in glory_render::profile::Phase::ALL {
                        if matches!(
                            phase,
                            glory_render::profile::Phase::Flush
                                | glory_render::profile::Phase::Observe
                                | glory_render::profile::Phase::Resources
                        ) {
                            continue;
                        }
                        assert_eq!(profile.snapshot().sample(phase).calls, 0);
                    }
                    app.clear()?;
                    settle(&mut raster)?;
                    assert_eq!(app.stats().nodes, 0);
                    raster.clear();
                    assert!(raster.pixels().is_empty());
                }
            }
        }
    }
    Ok(())
}
#[cfg(target_arch = "wasm32")]
fn main() {}
