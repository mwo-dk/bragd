COMMENT =		play, study and keep board games

GH_ACCOUNT =		mwo-dk
GH_PROJECT =		bragd
GH_TAGNAME =		v0.1.0

# The built frontend (gui/dist), a file of the release: npm cannot fetch during the build.
SITES.f =		https://github.com/mwo-dk/bragd/releases/download/${GH_TAGNAME}/
DISTFILES.f =		bragd-frontend-${GH_TAGNAME}.tar.gz

CATEGORIES =		games

HOMEPAGE =		https://github.com/mwo-dk/bragd

MAINTAINER =		Michael W. Olesen <michael.w.olesen@gmail.com>

# MIT
PERMIT_PACKAGE =	Yes

# Completed by `make port-lib-depends-check` for a release (README.md).
WANTLIB +=		${MODCARGO_WANTLIB}

MODULES =		devel/cargo

LIB_DEPENDS =		devel/libsoup3 \
			www/webkitgtk4,webkitgtk41 \
			x11/gnome/librsvg \
			x11/gtk+3

CONFIGURE_STYLE =	cargo

SEPARATE_BUILD =	Yes

MODCARGO_INSTALL_TARGET_PATHS =	gui/src-tauri crates/bragd-helper

post-extract:
	mv ${WRKDIR}/dist ${WRKSRC}/gui/dist

post-install:
	${INSTALL_DATA_DIR} ${PREFIX}/share/applications \
		${PREFIX}/share/icons/hicolor/128x128/apps
	${INSTALL_DATA} ${WRKSRC}/packaging/bsd/bragd.desktop \
		${PREFIX}/share/applications/
	${INSTALL_DATA} ${WRKSRC}/assets/brand/icons/128x128.png \
		${PREFIX}/share/icons/hicolor/128x128/apps/bragd.png

.include "crates.inc"

.include <bsd.port.mk>
