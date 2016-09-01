"""This tool fetches data from real verwalter and stores it in the folder
in a form that a (fake) verwalter can consume.
"""
import os
import sys
import json
import shutil
import asyncio
import aiohttp
import pathlib
import argparse


async def get_json(session, url):
    with aiohttp.Timeout(10):
        async with session.get(url) as response:
            assert response.status == 200
            return await response.json()


def options():
    ap = argparse.ArgumentParser()
    # TODO(tailhook) add a simpler `--add-n-peers 10` command
    ap.add_argument('hostnames', action='append', metavar='HOST',
        help="Host to fetch data from")
    ap.add_argument('-R', '--runtime-dir', metavar='DIR', type=pathlib.Path,
        help="Fetch runtime into specified dir "
             "(example: /etc/verwalter/runtime)."
             "WARNING: Directory will be cleaned.")
    ap.add_argument('-P', '--parent-dir', metavar='DIR', type=pathlib.Path,
        help="Fetch current schedule and put it as a parent schedule for a "
             "next run of verwalter (example: /var/lib/verwalter/schedule).")
    return ap.parse_args()


def main():
    opt = options()
    loop = asyncio.get_event_loop()
    with aiohttp.ClientSession(loop=loop) as session:
        while True:
            for host in opt.hostnames:
                try:
                    status = loop.run_until_complete(get_json(session,
                        'http://' + host + ':8379/v1/status.json'))
                    leader_host = status['leader']['name']
                except Exception as e:
                    print("Error:", e, file=sys.stderr)
                    error = e
                else:
                    break
            else:
                raise error

            if opt.runtime_dir:
                scheduler_input = loop.run_until_complete(get_json(session,
                    'http://' + leader_host + ':8379/v1/scheduler_input.json'))
                if not scheduler_input:
                    continue
                if opt.runtime_dir.exists():
                    for filename in os.listdir(str(opt.runtime_dir)):
                        shutil.rmtree(str(opt.runtime_dir / filename))
                for role, rdata in scheduler_input['runtime'].items():
                    rdir = opt.runtime_dir / role
                    rdir.mkdir(parents=True)
                    for version, vdata in rdata.items():
                        fname = rdir / (version + '.json')
                        with fname.with_suffix('.tmp').open('wt') as f:
                            json.dump(vdata, f)
                        os.rename(str(fname.with_suffix('.tmp')), str(fname))

            if opt.parent_dir:
                schedule = loop.run_until_complete(get_json(session,
                    'http://' + leader_host + ':8379/v1/schedule.json'))
                if not schedule:
                    continue
                opt.parent_dir.mkdir(parents=True, exist_ok=True)
                with (opt.parent_dir / 'schedule.json.tmp').open('wt') as f:
                    json.dump(schedule, f)
                os.rename(
                    str(opt.parent_dir / 'schedule.json.tmp'),
                    str(opt.parent_dir / 'schedule.json'))
            break


if __name__ == '__main__':
    main()
