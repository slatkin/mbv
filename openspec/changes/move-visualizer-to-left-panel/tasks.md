## 1. Left Panel Visualizer Rendering

- [x] 1.1 In `src/app/render/mod.rs:render_main()`, compute visualizer height from bottom of `left_content` and reduce `queue_area` height accordingly
- [x] 1.2 Call `render_visualizer(f, left_viz_area)` within the `!self.queue_column_collapsed` block, rendering in the bottom strip of the queue panel
- [x] 1.3 Visualizer lives within the queue panel bounds — queue content is shortened, not the whole left panel
- [x] 2.1 Run existing visualizer tests to confirm no regression (649 + 265 tests pass)
- [ ] 2.2 Visually verify: with visualizer enabled (`v` key), both left and right panels show synchronized visualizer strips at bottom
- [ ] 2.3 Visually verify: with queue column collapsed (`h` key), no left visualizer renders
- [ ] 2.4 Visually verify: with visualizer disabled, queue panel uses full height
