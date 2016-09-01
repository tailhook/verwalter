#!/usr/bin/env python3
"""Fake Cantal Server

This server is able to enulate large cluster with many peers, so that
you can run verwalter against it and solve scheduling cluster without actually
running it.
"""

import time
import json
import random
import argparse
from aiohttp import web

start_time = time.time()


async def make_peers(request):
    # We only emulate things that rotor-cantal supports
    peers = [{
        'id': "77985419c732412ea38b94db{:08d}".format(idx),
        'hostname': hostname,
        'name': hostname,
        'primary_addr': "192.168.255.{}:22682".format(idx),
        'addresses': ["192.168.255.{}:22682".format(idx)],
        'known_since': int(start_time * 1000),
        'last_report_direct': int((time.time() - random.random()) * 1000),
    } for idx, hostname in enumerate(request.app['options'].peers, 1)]
    return web.Response(
        body=json.dumps({'peers': peers}).encode('ascii'),
        content_type='application/json',
    )

async def hello(request):
    text = """
        <h1>Hello, I'm fake cantal!</h2>
        <p>I'm faking pings from the following hosts:</p>
        <ul>
            {}
        </ul>
        """.format('\n'.join('<li>{}</li>'.format(p)
                             for p in request.app['peers']))
    return web.Response(
        body=text.encode('ascii'),
        content_type='text/html',
    )


def options():
    ap = argparse.ArgumentParser()
    # TODO(tailhook) add a simpler `--add-n-peers 10` command
    ap.add_argument('--peers', nargs='*',
        help="List of hostnames for peers")
    return ap.parse_args()


def main():
    args = options()
    app = web.Application()
    app.router.add_route('GET', '/all_peers.json', make_peers)
    app['options'] = args
    web.run_app(app, port=22682)


if __name__ == '__main__':
    main()
