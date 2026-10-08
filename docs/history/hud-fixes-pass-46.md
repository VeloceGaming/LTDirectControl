# HUD / reader corrections, 0.46.0

## Purchase registry

The old code interpreted context[0] +0x1b8 as settings. AI bridge getter 0x2e84800 actually returns the simulation tick through that field. This explains why a valid executable identity still yielded no catalogue. Native buying at 0xea1670 reads its simulation context +8 as the settings owner, then settings +0x30 as a boxed Vec. AI buying consumer 0xdeb303 independently reads the same +8 field. Both exact instructions are now guarded in the reviewed 0.6.3 profile. No ABI/hash guards were removed.

The reader follows context +8 -> settings +0x30 -> boxed Vec, copies registered item getters inside the native AI callback, and reports stage/index/key on rejection. A synthetic regression test uses distinct simulation/settings pointers so the old tick chain cannot pass. Missing unrelated upgrade destinations no longer discard all effective entries; selected forecast paths still require complete known items. Forecast prices and native purchase parity remain a user game check.

## Construction picking

Native renderer name dispatch (0x27ecc00) compares generic `tower` and `nexus`, mapping `tower` to blue_tower resource at 0x3ca13d6. The previous art lookup expected side-specific resource names and fell back to 48x80 even for ordinary towers. Generic names now map to full base body metadata; tower is 31x63. Nexus counts as a structure even without the tower entity-kind flag. Runtime mod art overrides still take precedence.

The envelope uses modest lateral padding and an estimated eight-unit base portion below the ground origin. This base pivot is inferred from user screenshots, not proven from the native draw transform. It must be checked in the game. Corner brackets share the actual body picking envelope; height no longer inflates a ground ellipse. Champion/minion/monster selection policies remain as tested before this pass. These brackets are not alpha-based sprite outlines.

## HP and screen effect

HP fill is #2e7d46 with outlined white current/maximum numbers. Low HP warning previously reached only about 5% outer opacity at 20% HP. It now has a visible minimum below 25%, strengthens as HP falls, and fades without flashing. Cached native UI color nodes place it below the HUD. Native spawning/property success and mode transitions are logged. This replaces reliance on post-render draw commands; the earlier draw ordering was not conclusively established as the cause.

The viewport/minimap subtraction remains intact. Death shading uses the same native overlay; battlefield grayscale shader is unchanged. Disabling/releasing clears the warning. Nodes are retained and properties updated only when fields change. Native appearance/performance still require user testing.

## Tab preview

Separate review-tab-46.html, tab-46.css and compare-tab-46.html preserve all earlier previews. Proposed panel: team halves, availability/respawn portraits, 22px KDA, 21px CS/gold and six 32px item slots. Controlled row has a thin yellow edge. Browser validation checks 10 rows, 60 slots, no cell overflow, toggle and dead availability. Gold/row values are sample data. The installed Tab panel remains vanilla until approval.

227 Rust tests (209 probe +18 core), Clippy, format, release build and exact native/package checks are required. Native gameplay/rendering is not verified by these checks.
