# Tab and purchase display correction (0.48.0)

The 0.47 native panel closed using a direct visibility setter but opened through the cached property writer. The cache retained visible=true after the root was hidden, suppressing later show operations. Both transitions now use the same property writer; unchanged setters remain suppressed and failed writes do not update the cache. Regression coverage exercises repeated close/open transitions through the writer with a simulated host.

KDA used six-digit colour tags although the game's established rich-text format uses eight-digit RGBA. Tags now include alpha and the string ends with a reset. The user cannot identify the erroneous Tab field from the first test, so that report is not conclusively attributed to KDA.

The latest purchase log shows soldiers_longsword upgrading along two valid routes: bf_sword costs 450 and ruinous_blade costs 800. With 254 gold the old range was 196–546; it wrapped in the 58px purchase label. No price arithmetic or buyer logic changes. The HUD now displays 196+, meaning minimum shortfall; tooltip rows show full price and additional gold for each branch. Known single purchases and ready states retain their existing behavior. Branch uncertainty is not presented as a predetermined purchase.

The accepted geometry and adaptive inventory behavior remain unchanged. Native rendering and the full panel lifecycle require user testing.
