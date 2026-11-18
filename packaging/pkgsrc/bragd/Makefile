# $NetBSD$

DISTNAME=	bragd-0.1.0
CATEGORIES=	games
MASTER_SITES=	${MASTER_SITE_GITHUB:=mwo-dk/}
GITHUB_TAG=	v${PKGVERSION_NOREV}

# The built frontend (gui/dist), a file of the release: npm cannot fetch during the build.
FRONTEND=	bragd-frontend-v${PKGVERSION_NOREV}.tar.gz
DISTFILES=	${DEFAULT_DISTFILES} ${FRONTEND}
SITES.${FRONTEND}=	https://github.com/mwo-dk/bragd/releases/download/v${PKGVERSION_NOREV}/

MAINTAINER=	michael.w.olesen@gmail.com
HOMEPAGE=	https://github.com/mwo-dk/bragd
COMMENT=	Play, study and keep board games: chess, go, shogi and more
LICENSE=	mit

USE_TOOLS+=	pkg-config

.include "cargo-depends.mk"

CARGO_ARGS+=	--package bragd-gui --package bragd-helper

INSTALLATION_DIRS=	bin share/applications share/icons/hicolor/128x128/apps

post-extract:
	${MV} ${WRKDIR}/dist ${WRKSRC}/gui/dist

do-install:
	${INSTALL_PROGRAM} ${WRKSRC}/target/release/bragd-gui \
		${WRKSRC}/target/release/bragd-helper ${DESTDIR}${PREFIX}/bin
	${INSTALL_DATA} ${WRKSRC}/packaging/bsd/bragd.desktop \
		${DESTDIR}${PREFIX}/share/applications
	${INSTALL_DATA} ${WRKSRC}/assets/brand/icons/128x128.png \
		${DESTDIR}${PREFIX}/share/icons/hicolor/128x128/apps/bragd.png

.include "../../lang/rust/cargo.mk"
.include "../../graphics/librsvg/buildlink3.mk"
.include "../../net/libsoup3/buildlink3.mk"
.include "../../www/webkit-gtk41/buildlink3.mk"
.include "../../x11/gtk3/buildlink3.mk"
.include "../../mk/bsd.pkg.mk"
