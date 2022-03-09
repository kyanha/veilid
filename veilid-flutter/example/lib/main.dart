import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:veilid/veilid.dart';
import 'package:flutter_loggy/flutter_loggy.dart';
import 'package:loggy/loggy.dart';

import 'config.dart';

// Loggy tools
const LogLevel traceLevel = LogLevel('trace', 1);

extension TraceLoggy on Loggy {
  void trace(dynamic message, [Object? error, StackTrace? stackTrace]) =>
      log(traceLevel, message, error, stackTrace);
}

LogOptions getLogOptions(LogLevel? level) {
  return LogOptions(
    level ?? LogLevel.all,
    stackTraceLevel: LogLevel.error,
  );
}

void setRootLogLevel(LogLevel? level) {
  print("setRootLogLevel: $level");
  Loggy('').level = getLogOptions(level);
}

void initLoggy() {
  Loggy.initLoggy(
    logPrinter: StreamPrinter(
      const PrettyDeveloperPrinter(),
    ),
    logOptions: getLogOptions(null),
  );
}

// Entrypoint
void main() {
  WidgetsFlutterBinding.ensureInitialized();

  initLoggy();

  runApp(MaterialApp(
      title: 'Veilid Plugin Demo',
      theme: ThemeData(
        primarySwatch: Colors.blue,
        visualDensity: VisualDensity.adaptivePlatformDensity,
      ),
      home: const MyApp()));
}

// Main App
class MyApp extends StatefulWidget {
  const MyApp({Key? key}) : super(key: key);

  @override
  State<MyApp> createState() => _MyAppState();
}

class _MyAppState extends State<MyApp> with UiLoggy {
  String _veilidVersion = 'Unknown';
  Stream<VeilidUpdate>? _updateStream;
  Future<void>? _updateProcessor;

  @override
  void initState() {
    super.initState();
    initPlatformState();
  }

  // Platform messages are asynchronous, so we initialize in an async method.
  Future<void> initPlatformState() async {
    String veilidVersion;
    // Platform messages may fail, so we use a try/catch PlatformException.
    // We also handle the message potentially returning null.
    try {
      veilidVersion = Veilid.instance.veilidVersionString();
    } on PlatformException {
      veilidVersion = 'Failed to get veilid version.';
    }
    loggy.error("Error test");
    loggy.warning("Warning test");
    loggy.info("Info test");
    loggy.debug("Debug test");
    loggy.trace("Trace test");

    // If the widget was removed from the tree while the asynchronous platform
    // message was in flight, we want to discard the reply rather than calling
    // setState to update our non-existent appearance.
    if (!mounted) return;

    setState(() {
      _veilidVersion = veilidVersion;
    });
  }

  Future<void> processUpdateLog(VeilidUpdateLog update) async {
    switch (update.logLevel) {
      case VeilidLogLevel.error:
        loggy.error(update.message);
        break;
      case VeilidLogLevel.warn:
        loggy.warning(update.message);
        break;
      case VeilidLogLevel.info:
        loggy.info(update.message);
        break;
      case VeilidLogLevel.debug:
        loggy.debug(update.message);
        break;
      case VeilidLogLevel.trace:
        loggy.trace(update.message);
        break;
    }
  }

  Future<void> processUpdates() async {
    var stream = _updateStream;
    if (stream != null) {
      await for (final update in stream) {
        if (update is VeilidUpdateLog) {
          await processUpdateLog(update);
        } else {
          loggy.trace("Update: " + update.toString());
        }
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final ButtonStyle buttonStyle =
        ElevatedButton.styleFrom(textStyle: const TextStyle(fontSize: 20));

    return Scaffold(
        appBar: AppBar(
          title: Text('Veilid Plugin Version $_veilidVersion'),
        ),
        body: Column(children: [
          Expanded(
              child: Container(
            color: ThemeData.dark().scaffoldBackgroundColor,
            height: MediaQuery.of(context).size.height * 0.4,
            child: LoggyStreamWidget(logLevel: loggy.level.logLevel),
          )),
          Container(
              padding: const EdgeInsets.fromLTRB(8, 8, 8, 12),
              child: Row(children: [
                ElevatedButton(
                  style: buttonStyle,
                  onPressed: _updateStream != null
                      ? null
                      : () async {
                          var updateStream = Veilid.instance.startupVeilidCore(
                              await getDefaultVeilidConfig());
                          setState(() {
                            _updateStream = updateStream;
                            _updateProcessor = processUpdates();
                          });
                        },
                  child: const Text('Startup'),
                ),
                ElevatedButton(
                  style: buttonStyle,
                  onPressed: _updateStream == null
                      ? null
                      : () async {
                          await Veilid.instance.shutdownVeilidCore();
                          if (_updateProcessor != null) {
                            await _updateProcessor;
                          }
                          setState(() {
                            _updateProcessor = null;
                            _updateStream = null;
                          });
                        },
                  child: const Text('Shutdown'),
                ),
              ])),
          Row(children: [
            Expanded(
                child: TextField(
                    decoration: const InputDecoration(
                        border: OutlineInputBorder(),
                        labelText: 'Debug Command'),
                    textInputAction: TextInputAction.send,
                    onSubmitted: (String v) async {
                      loggy.debug(await Veilid.instance.debug(v));
                    })),
            DropdownButton<LogLevel>(
                value: loggy.level.logLevel,
                onChanged: (LogLevel? newLevel) {
                  setState(() {
                    setRootLogLevel(newLevel);
                  });
                },
                items: const [
                  DropdownMenuItem<LogLevel>(
                      value: LogLevel.error, child: Text("Error")),
                  DropdownMenuItem<LogLevel>(
                      value: LogLevel.warning, child: Text("Warning")),
                  DropdownMenuItem<LogLevel>(
                      value: LogLevel.info, child: Text("Info")),
                  DropdownMenuItem<LogLevel>(
                      value: LogLevel.debug, child: Text("Debug")),
                  DropdownMenuItem<LogLevel>(
                      value: traceLevel, child: Text("Trace")),
                  DropdownMenuItem<LogLevel>(
                      value: LogLevel.all, child: Text("All")),
                ])
          ]),
        ]));
  }
}
