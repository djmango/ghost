import { useState, useEffect } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { trace, info, error, attachConsole } from '@tauri-apps/plugin-log';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Loader2 } from "lucide-react";
import "./styles.css";

type AlertType = 'error' | 'success';
interface Alert {
  type: AlertType;
  message: string;
}

interface RecordingAnalysis {
  total_duration: number;
  total_events: number;
  event_counts: Record<string, number>;
  total_mouse_distance: number;
}

export default function MainScreen() {
  const [isRecording, setIsRecording] = useState<boolean>(false);
  const [alert, setAlert] = useState<Alert | null>(null);
  const [analysis, setAnalysis] = useState<RecordingAnalysis | null>(null);

  useEffect(() => {
    // Set theme based on system preference
    invoke("plugin:theme|set_theme", {
      theme: "auto",
    });

    async function setupLogs() {
      await attachConsole();
      trace('This is a trace message');
      info('This is an info message');
      error('This is an error message');
    }
    setupLogs();

    const unlistenComplete = listen('recording_complete', (event) => {
      info(`Recording completed: ${event.payload}`);
      setAlert({ type: 'success', message: `Recording completed successfully. Saved to ${event.payload}` });
      setIsRecording(false);
      analyzeRecording();
    });

    const unlistenError = listen('recording_error', (event) => {
      error(`Recording error: ${event.payload}`);
      setAlert({ type: 'error', message: `Recording failed: ${event.payload}` });
      setIsRecording(false);
    });

    return () => {
      unlistenComplete.then(f => f());
      unlistenError.then(f => f());
    };
  }, []);

  const startRecording = async () => {
    try {
      setIsRecording(true);
      setAlert(null);
      setAnalysis(null);
      await invoke('start_recording');
    } catch (error: any) {
      error(`Failed to start recording: ${error}`);
      setAlert({ type: 'error', message: 'Failed to start recording' });
      setIsRecording(false);
    }
  };

  const stopRecording = async () => {
    try {
      await invoke('stop_recording');
      // The recording_complete event will handle the rest
    } catch (error: any) {
      error(`Failed to stop recording: ${error}`);
      setAlert({ type: 'error', message: 'Failed to stop recording' });
      setIsRecording(false);
    }
  };

  const analyzeRecording = async () => {
    try {
      const result = await invoke('analyze_recording');
      setAnalysis(result as RecordingAnalysis);
    } catch (error: any) {
      error(`Failed to analyze recording: ${error}`);
      setAlert({ type: 'error', message: 'Failed to analyze recording' });
    }
  };

  return (
    <div className="min-h-screen bg-white dark:bg-gray-900 text-black dark:text-white p-8 flex flex-col items-center justify-start">
      <div className="max-w-4xl w-full flex flex-col items-center text-center">
        <div className="flex items-center mb-12">
          <img src="/logo.svg" alt="i.inc logo" className="w-16 h-16" />
          <h1 className="text-4xl font-bold ml-4">i.inc Desktop Event Recorder</h1>
        </div>

        {alert && (
          <Alert variant={alert.type === 'error' ? "destructive" : "default"} className="mb-6">
            <AlertDescription>{alert.message}</AlertDescription>
          </Alert>
        )}

        <Card className="w-full mb-8 bg-white dark:bg-gray-800">
          <CardHeader>
            <CardTitle>Recording Controls</CardTitle>
          </CardHeader>
          <CardContent className="flex justify-center space-x-4">
            <Button
              variant="outline"
              onClick={startRecording}
              disabled={isRecording}
            // className="bg-blue-500 hover:bg-blue-600 text-white"
            >
              {isRecording ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Recording...
                </>
              ) : (
                'Start Recording'
              )}
            </Button>
            <Button
              onClick={stopRecording}
              disabled={!isRecording}
              variant="outline"
              className="border-blue-500 text-blue-500 hover:bg-blue-100 dark:hover:bg-blue-900"
            >
              Stop Recording
            </Button>
          </CardContent>
        </Card>

        {analysis && (
          <Card className="w-full bg-white dark:bg-gray-800">
            <CardHeader>
              <CardTitle>Recording Analysis</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <h3 className="font-semibold">Total Duration</h3>
                  <p>{analysis.total_duration} seconds</p>
                </div>
                <div>
                  <h3 className="font-semibold">Total Events</h3>
                  <p>{analysis.total_events}</p>
                </div>
                <div>
                  <h3 className="font-semibold">Mouse Distance</h3>
                  <p>{analysis.total_mouse_distance.toFixed(2)} pixels</p>
                </div>
                <div>
                  <h3 className="font-semibold">Event Breakdown</h3>
                  <ul className="list-disc list-inside">
                    {Object.entries(analysis.event_counts).map(([event, count]) => (
                      <li key={event}>{event}: {count}</li>
                    ))}
                  </ul>
                </div>
              </div>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
}
