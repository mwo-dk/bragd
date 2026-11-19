PORTNAME=	bragd
DISTVERSIONPREFIX=	v
DISTVERSION=	0.1.0
CATEGORIES=	games
MASTER_SITES=	https://github.com/mwo-dk/bragd/releases/download/v${DISTVERSION}/:frontend
DISTFILES=	bragd-frontend-v${DISTVERSION}.tar.gz:frontend

MAINTAINER=	michael.w.olesen@gmail.com
COMMENT=	Play, study and keep board games: chess, go, shogi, xiangqi and more
WWW=		https://github.com/mwo-dk/bragd

LICENSE=	MIT
LICENSE_FILE=	${WRKSRC}/LICENSE

LIB_DEPENDS=	libharfbuzz.so:print/harfbuzz \
		librsvg-2.so:graphics/librsvg2-rust \
		libsoup-3.0.so:devel/libsoup3 \
		libwebkit2gtk-4.1.so:www/webkit2-gtk@41

USES=		cargo gnome pkgconfig
USE_GITHUB=	yes
GH_ACCOUNT=	mwo-dk
USE_GNOME=	cairo gdkpixbuf glib20 gtk30 pango

# The window embeds the built frontend (gui/dist), the release's bragd-frontend file: npm
# cannot fetch during a ports build.
CARGO_BUILD_ARGS=	--package bragd-gui --package bragd-helper
CARGO_INSTALL_PATH=	gui/src-tauri crates/bragd-helper
CARGO_TEST_ARGS=	--package bragd-core

PLIST_FILES=	bin/bragd-gui \
		bin/bragd-helper \
		share/applications/bragd.desktop \
		share/icons/hicolor/128x128/apps/bragd.png

post-extract:
	${MV} ${WRKDIR}/dist ${WRKSRC}/gui/dist

post-install:
	${INSTALL_DATA} ${WRKSRC}/packaging/bsd/bragd.desktop \
		${STAGEDIR}${PREFIX}/share/applications/
	${MKDIR} ${STAGEDIR}${PREFIX}/share/icons/hicolor/128x128/apps
	${INSTALL_DATA} ${WRKSRC}/assets/brand/icons/128x128.png \
		${STAGEDIR}${PREFIX}/share/icons/hicolor/128x128/apps/bragd.png

.include <bsd.port.mk>
