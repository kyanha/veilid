# Routing context veilid tests

import asyncio
import random
import sys
import os

import pytest
import veilid
from veilid.types import OperationId

from .conftest import server_info

##################################################################


@pytest.mark.asyncio
async def test_routing_contexts(api_connection: veilid.VeilidAPI):
    rc = await api_connection.new_routing_context()
    async with rc:
        rcp = await rc.with_privacy(release=False)
        async with rcp:
            rcps = await rcp.with_sequencing(veilid.Sequencing.ENSURE_ORDERED, release=False)
            async with rcps:
                rcpscp = await rcps.with_custom_privacy(veilid.Stability.RELIABLE, release=False)
                await rcpscp.release()


@pytest.mark.asyncio
async def test_routing_context_app_message_loopback():
    # Seriously, mypy?
    app_message_queue: asyncio.Queue = asyncio.Queue()

    async def app_message_queue_update_callback(update: veilid.VeilidUpdate):
        if update.kind == veilid.VeilidUpdateKind.APP_MESSAGE:
            await app_message_queue.put(update)

    hostname, port = server_info()
    api = await veilid.json_api_connect(
        hostname, port, app_message_queue_update_callback
    )
    async with api:
        # purge routes to ensure we start fresh
        await api.debug("purge routes")

        # make a routing context that uses a safety route
        rc = await (await api.new_routing_context()).with_privacy()
        async with rc:

            # make a new local private route
            prl, blob = await api.new_private_route()

            # import it as a remote route as well so we can send to it
            prr = await api.import_remote_private_route(blob)

            # send an app message to our own private route
            message = b"abcd1234"
            await rc.app_message(prr, message)

            # we should get the same message back
            update: veilid.VeilidUpdate = await asyncio.wait_for(
                app_message_queue.get(), timeout=10
            )

            assert isinstance(update.detail, veilid.VeilidAppMessage)
            assert update.detail.message == message


@pytest.mark.asyncio
async def test_routing_context_app_call_loopback():
    app_call_queue: asyncio.Queue = asyncio.Queue()

    async def app_call_queue_update_callback(update: veilid.VeilidUpdate):
        if update.kind == veilid.VeilidUpdateKind.APP_CALL:
            await app_call_queue.put(update)

    hostname, port = server_info()
    api = await veilid.json_api_connect(hostname, port, app_call_queue_update_callback)
    async with api:
        # purge routes to ensure we start fresh
        await api.debug("purge routes")

        # make a routing context that uses a safety route
        rc = await (await api.new_routing_context()).with_privacy()
        async with rc:

            # make a new local private route
            prl, blob = await api.new_private_route()

            # import it as a remote route as well so we can send to it
            prr = await api.import_remote_private_route(blob)

            # send an app message to our own private route
            request = b"abcd1234"
            app_call_task = asyncio.create_task(
                rc.app_call(prr, request), name="app call task"
            )

            # we should get the same request back
            update: veilid.VeilidUpdate = await asyncio.wait_for(
                app_call_queue.get(), timeout=10
            )
            appcall = update.detail

            assert isinstance(appcall, veilid.VeilidAppCall)
            assert appcall.message == request

            # now we reply to the request
            reply = b"qwer5678"
            await api.app_call_reply(appcall.call_id, reply)

            # now we should get the reply from the call
            result = await app_call_task
            assert result == reply


@pytest.mark.asyncio
async def test_routing_context_app_message_loopback_big_packets():

    app_message_queue: asyncio.Queue = asyncio.Queue()

    global got_message
    got_message = 0
    async def app_message_queue_update_callback(update: veilid.VeilidUpdate):
        if update.kind == veilid.VeilidUpdateKind.APP_MESSAGE:
            global got_message
            got_message += 1
            sys.stdout.write("{} ".format(got_message))
            await app_message_queue.put(update)

    sent_messages: set[bytes] = set()

    hostname, port = server_info()
    api = await veilid.json_api_connect(
        hostname, port, app_message_queue_update_callback
    )
    async with api:
        # purge routes to ensure we start fresh
        await api.debug("purge routes")

        # make a routing context that uses a safety route
        rc = await (await (await api.new_routing_context()).with_privacy()).with_sequencing(veilid.Sequencing.ENSURE_ORDERED)
        async with rc:

            # make a new local private route
            prl, blob = await api.new_private_route()

            # import it as a remote route as well so we can send to it
            prr = await api.import_remote_private_route(blob)

            # do this test 1000 times
            for _ in range(1000):

                # send a random sized random app message to our own private route
                message = random.randbytes(random.randint(0, 32768))
                await rc.app_message(prr, message)

                sent_messages.add(message)

            # we should get the same messages back
            print(len(sent_messages))
            for n in range(len(sent_messages)):
                print(n)
                update: veilid.VeilidUpdate = await asyncio.wait_for(
                    app_message_queue.get(), timeout=10
                )
                assert isinstance(update.detail, veilid.VeilidAppMessage)

                assert update.detail.message in sent_messages

@pytest.mark.asyncio
async def test_routing_context_app_call_loopback_big_packets():
    global got_message
    got_message = 0
    
    app_call_queue: asyncio.Queue = asyncio.Queue()

    async def app_call_queue_update_callback(update: veilid.VeilidUpdate):
        if update.kind == veilid.VeilidUpdateKind.APP_CALL:
            await app_call_queue.put(update)

    async def app_call_queue_task_handler(api: veilid.VeilidAPI):
        while True:
            update = await app_call_queue.get()
            
            global got_message
            got_message += 1
            
            sys.stdout.write("{} ".format(got_message))
            sys.stdout.flush()

            await api.app_call_reply(update.detail.call_id, update.detail.message)
        
    hostname, port = server_info()
    api = await veilid.json_api_connect(
        hostname, port, app_call_queue_update_callback
    )
    async with api:
        # purge routes to ensure we start fresh
        await api.debug("purge routes")

        app_call_task = asyncio.create_task(
            app_call_queue_task_handler(api), name="app call task"
        )

        # make a routing context that uses a safety route
        rc = await (await (await api.new_routing_context()).with_privacy()).with_sequencing(veilid.Sequencing.ENSURE_ORDERED)
        async with rc:

            # make a new local private route
            prl, blob = await api.new_private_route()

            # import it as a remote route as well so we can send to it
            prr = await api.import_remote_private_route(blob)

            # do this test 10 times
            for _ in range(10):

                # send a random sized random app message to our own private route
                message = random.randbytes(random.randint(0, 32768))
                out_message = await rc.app_call(prr, message)

                assert message == out_message
        
        app_call_task.cancel()


@pytest.mark.skipif(os.getenv("NOSKIP")!="1", reason="unneeded test, only for performance check")
@pytest.mark.asyncio
async def test_routing_context_app_message_loopback_bandwidth():

    app_message_queue: asyncio.Queue = asyncio.Queue()

    async def app_message_queue_update_callback(update: veilid.VeilidUpdate):
        if update.kind == veilid.VeilidUpdateKind.APP_MESSAGE:
            await app_message_queue.put(True)

    hostname, port = server_info()
    api = await veilid.json_api_connect(
        hostname, port, app_message_queue_update_callback
    )
    async with api:
        # purge routes to ensure we start fresh
        await api.debug("purge routes")

        # make a routing context that uses a safety route
        #rc = await (await (await api.new_routing_context()).with_privacy()).with_sequencing(veilid.Sequencing.ENSURE_ORDERED)
        #rc = await (await api.new_routing_context()).with_privacy()
        rc = await api.new_routing_context()
        async with rc:

            # make a new local private route
            prl, blob = await api.new_private_route()

            # import it as a remote route as well so we can send to it
            prr = await api.import_remote_private_route(blob)

            # do this test 1000 times
            message = random.randbytes(16384)
            for _ in range(10000):

                # send a random sized random app message to our own private route
                await rc.app_message(prr, message)

            # we should get the same number of messages back (not storing all that data)
            for _ in range(10000):
                await asyncio.wait_for(
                    app_message_queue.get(), timeout=10
                )
