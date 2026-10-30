# Copy game code from coxswain instead of sharing crates

Bragd and coxswain stay separate apps, and both keep board-game support for now. Bragd copies coxswain's game, engine, container, i18n and update code once and owns it from then on. It does not depend on shared crates. Shared crates would tie two release trains together while Bragd changes fast and coxswain's game support stays mostly still. We accept that the copies will drift. Open game issues in coxswain stay there, and Bragd files its own.
