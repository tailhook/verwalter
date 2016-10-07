PREFIX ?= /usr
DESTDIR ?=

all: bin js

release: bin-release js-release

bin:
	cargo build

bin-release:
	cargo build --release


js:
	cd frontend; webpack

js-release:
	# can't do --optimize-minimize because we use ES6 syntax, we use babili
	cd frontend; NODE_ENV=production webpack

install:
	install -D -m 755 target/release/verwalter $(DESTDIR)$(PREFIX)/bin/verwalter
	install -D -m 755 target/release/verwalter_render \
		$(DESTDIR)$(PREFIX)/bin/verwalter_render
	install -d $(DESTDIR)$(PREFIX)/share/verwalter
	cp -R public $(DESTDIR)$(PREFIX)/share/verwalter/frontend


install-systemd:
	install -D ./systemd.service $(DESTDIR)$(PREFIX)/lib/systemd/system/verwalter.service

install-upstart:
	install -D ./upstart.conf $(DESTDIR)/etc/init/verwalter.conf

ubuntu-packages: version:=$(shell git describe --dirty)
ubuntu-packages: codename:=$(shell lsb_release --codename --short)
ubuntu-packages:
	rm -rf pkg
	rm -rf target/release
	bulk with-version "$(version)" cargo build --release
	make install DESTDIR=/work/pkg
	bulk pack --package-version="$(version)+$(codename)1"

ubuntu-verwalter_render-package: version:=$(shell git describe --dirty)
ubuntu-verwalter_render-package:
	-rm -rf pkg
	-rm -rf target/x86_64-unknown-linux-musl/release/verwalter_render
	bulk with-version "$(version)" \
		cargo build --target=x86_64-unknown-linux-musl --bin=verwalter_render
	install -D ./target/x86_64-unknown-linux-musl/release/verwalter_render \
		pkg/usr/bin/verwalter_render
	bulk pack --config=bulk-render.yaml --package-version="$(version)"

.PHONY: bin js
