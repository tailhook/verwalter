PREFIX ?= /usr
DESTDIR ?=
export VERWALTER_VERSION := $(shell git describe)

all: bin js

bin:
	cargo build

js:
	cd frontend; webpack

install:
	install -D -m 755 target/debug/verwalter $(DESTDIR)$(PREFIX)/bin/verwalter
	install -D -m 755 target/debug/verwalter_render \
		$(DESTDIR)$(PREFIX)/bin/verwalter_render
	install -d $(DESTDIR)$(PREFIX)/share/verwalter
	cp -R public $(DESTDIR)$(PREFIX)/share/verwalter/frontend

.PHONY: bin js
