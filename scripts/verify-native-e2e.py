#!/usr/bin/env python3
"""Load the built plugin through its real C ABI and exercise loopback HTTP."""
import argparse
import ctypes
import json
import pathlib
import socket
import tempfile
import urllib.error
import urllib.parse
import urllib.request
import zipfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    package = pathlib.Path(args.package).resolve()
    output = pathlib.Path(args.output).resolve()
    with tempfile.TemporaryDirectory(prefix="fileshare-abi-e2e-") as temporary:
        import os
        root = pathlib.Path(temporary).resolve()
        # 每次运行独占配置，含原生依赖读取 HOME 时也不会碰真实用户数据。
        home = root / "ffi-home"
        home.mkdir()
        os.environ["HOME"] = str(home)
        os.environ["OTOOLS_HOME"] = str(home)
        installed = (root / "plugin").resolve()
        installed.mkdir()
        with zipfile.ZipFile(package) as archive:
            for entry in archive.infolist():
                destination = (installed / entry.filename).resolve()
                if not destination.is_relative_to(installed):
                    raise RuntimeError("Unsafe package path")
            archive.extractall(installed)
        library = ctypes.CDLL(str(installed / "lib/macOS.dylib"))
        invoke = library.otools_plugin_invoke
        invoke.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
        invoke.restype = ctypes.c_void_p
        free = library.otools_plugin_free
        free.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
        free.restype = None

        def call(method, payload=None):
            request = json.dumps({"method": method, "payload": payload, "pluginDir": str(installed)}).encode()
            buffer = ctypes.create_string_buffer(request)
            length = ctypes.c_size_t()
            pointer = invoke(buffer, len(request), ctypes.byref(length))
            assert pointer, method
            try:
                response = json.loads(ctypes.string_at(pointer, length.value))
            finally:
                free(pointer, length.value)
            assert response["ok"], (method, response)
            return response.get("data")

        # 上面已在加载动态库前隔离 HOME。
        upload = home / "Downloads"
        upload.mkdir(parents=True, exist_ok=True)
        source = root / "断点 下载.txt"
        source.write_bytes(b"0123456789ABCDEFGHIJ")
        folder = root / "shared-folder"
        folder.mkdir()
        (folder / "hello.txt").write_text("zip hello")
        with socket.socket() as probe:
            probe.bind(("127.0.0.1", 0))
            port = probe.getsockname()[1]
        settings = {
            "uploadPath": str(upload), "port": port, "authEnable": False,
            "tusEnable": True, "chunkSize": 1, "autoStart": False,
            "language": "zh", "allowAnonymousUpload": True,
        }
        assert call("ping") == "pong"
        call("updateSetting", settings)
        call("addFile", {"name": source.name, "path": str(source)})
        call("addFile", {"name": folder.name, "path": str(folder)})
        started = call("startServer")
        base = f"http://127.0.0.1:{port}"
        results = {"abi_ping": True, "started": started["success"]}

        def http(path, data=None, method=None, headers=None):
            request = urllib.request.Request(base + path, data=data, method=method, headers=headers or {})
            return urllib.request.urlopen(request, timeout=10)

        try:
            with http("/") as response:
                assert b"File Share" in response.read()
            with http("/some/deep/link") as response:
                assert response.status == 200 and b"File Share" in response.read()
            results["static_and_spa"] = True
            filename = urllib.parse.quote(source.name)
            with http("/api/download?filename=" + filename, headers={"Range": "bytes=5-"}) as response:
                assert response.status == 206
                assert response.headers["Content-Range"] == "bytes 5-19/20"
                assert response.read() == b"56789ABCDEFGHIJ"
            results["unicode_range_download"] = True
            with http("/api/download?filename=shared-folder&timestamp=e2e") as response:
                import io
                with zipfile.ZipFile(io.BytesIO(response.read())) as archive:
                    assert archive.read("hello.txt") == b"zip hello"
            results["zip_download"] = True
            payload = b'--test\r\nContent-Disposition: form-data; name="file"; filename="upload.txt"\r\n\r\nuploaded\r\n--test--\r\n'
            with http("/api/addFile", payload, "POST", {"Content-Type": "multipart/form-data; boundary=test"}) as response:
                assert json.load(response)["code"] == 200
            assert (upload / "upload.txt").read_bytes() == b"uploaded"
            results["multipart_upload"] = True
            import base64
            metadata = "filename " + base64.b64encode(b"resumed.txt").decode()
            with http("/api/uploads", b"", "POST", {"Tus-Resumable": "1.0.0", "Upload-Length": "10", "Upload-Metadata": metadata}) as response:
                assert response.status == 201
                location = response.headers["Location"]
            headers = {"Tus-Resumable": "1.0.0", "Upload-Offset": "0", "Content-Type": "application/offset+octet-stream"}
            with http(location, b"01234", "PATCH", headers) as response:
                assert response.status == 204 and response.headers["Upload-Offset"] == "5"
            with http(location, method="HEAD", headers={"Tus-Resumable": "1.0.0"}) as response:
                assert response.headers["Upload-Offset"] == "5"
            headers["Upload-Offset"] = "5"
            with http(location, b"56789", "PATCH", headers) as response:
                assert response.status == 204
            assert (upload / "resumed.txt").read_bytes() == b"0123456789"
            results["tus_resume"] = True
            call("addAccount", {"username": "e2e", "password": "only-local-test", "permissions": {"download": True, "uploadFile": False, "uploadText": False}})
            settings["authEnable"] = True
            call("updateSetting", settings)
            with http("/api/files") as response:
                assert json.load(response)["code"] == 401
            login = json.dumps({"username": "e2e", "password": "only-local-test"}).encode()
            with http("/api/login", login, "POST", {"Content-Type": "application/json"}) as response:
                login_data = json.load(response)
                assert login_data["code"] == 200
                token = login_data["data"]["Authorization"]
            with http("/api/files", headers={"Authorization": token}) as response:
                assert json.load(response)["code"] == 200
            with http("/api/addText", b'{"message":"denied"}', "POST", {"Authorization": token, "Content-Type": "application/json"}) as response:
                assert json.load(response)["code"] == 403
            results["account_auth_and_write_permissions"] = True
        finally:
            call("stopServer")
            assert call("getServerStatus") == "stop"
            call("__otools_shutdown__")
        try:
            connection = socket.create_connection(("127.0.0.1", port), timeout=1)
        except ConnectionRefusedError:
            pass
        else:
            connection.close()
            raise AssertionError("listener still accepting connections after shutdown")
        results["shutdown_releases_listener"] = True
        output.write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
        print(json.dumps(results, ensure_ascii=False))


if __name__ == "__main__":
    main()
