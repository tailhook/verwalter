PREFIX ?= /usr
DESTDIR ?=

all: bin js

bin:
	cargo build

js:
	cd frontend; webpack

install:
	install -D -m 755 target/debug/verwalter $(DESTDIR)$(PREFIX)/bin/verwalter
	install -d $(DESTDIR)$(PREFIX)/share/verwalter
	cp -R public $(DESTDIR)$(PREFIX)/share/verwalter/frontend

.PHONY: bin js
