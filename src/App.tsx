import React, { useState, useEffect } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { trace, info, error, attachConsole } from '@tauri-apps/plugin-log';
import "./styles.css";

type Monitor = string;

type AlertType = 'error' | 'success';

interface Alert {
  type: AlertType;
  message: string;
}

const MainScreen: React.FC = () => {
  const [monitors, setMonitors] = useState<Monitor[]>([]);
  const [isRecording, setIsRecording] = useState<boolean>(false);
  const [recordingDuration, setRecordingDuration] = useState<number>(10);
  const [alert, setAlert] = useState<Alert | null>(null);


  useEffect(() => {
    async function setupLogs() {
      // const detach = await attachConsole();
      await attachConsole();
      trace('This is a trace message');
      info('This is an info message');
      error('This is an error message');
    }

    setupLogs();

    async function fetchMonitors() {
      try {
        const monitorNames = await invoke<string[]>('get_monitors');
        setMonitors(monitorNames);
      } catch (error: any) {
        // error('Failed to fetch monitors:', error);
        error(`Failed to fetch monitors: ${error}`);
        setAlert({ type: 'error', message: 'Failed to fetch monitors' });
      }
    }

    fetchMonitors();


    // Listen for the recording_complete event
    const unlistenComplete = listen('recording_complete', (event) => {
      info(`Recording completed: ${event.payload}`)
      setAlert({ type: 'success', message: `Recording completed successfully. Saved to ${event.payload}` });
      setIsRecording(false);
    });

    // Listen for the recording_error event
    const unlistenError = listen('recording_error', (event) => {
      error(`Recording error: ${event.payload}`);
      setAlert({ type: 'error', message: `Recording failed: ${event.payload}` });
      setIsRecording(false);
    });

    // Cleanup listeners on component unmount
    return () => {
      unlistenComplete.then(f => f());
      unlistenError.then(f => f());
    };
  }, []);

  const startRecording = async () => {
    try {
      setIsRecording(true);
      setAlert(null); // Clear any previous alerts
      await invoke('start_recording', { duration: recordingDuration });
      // The recording process has started, but we don't know if it's successful yet.
      // We'll wait for the recording_complete or recording_error event.
    } catch (error: any) {
      error(`Failed to start recording: ${error}`);
      setAlert({ type: 'error', message: 'Failed to start recording' });
      setIsRecording(false);
    }
  };

  return (
    <div style={{
      minHeight: '100vh',
      backgroundColor: 'white',
      color: '#1a202c',
      padding: '2rem',
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      justifyContent: 'center'
    }}>
      <div style={{
        maxWidth: '64rem',
        width: '100%',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        textAlign: 'center'
      }}>
        <h1>Welcome to i.inc!</h1>
        <h2>Monitor Name: {monitors}</h2>

        <div style={{ display: 'flex', alignItems: 'center', marginBottom: '3rem' }}>
          <img src="/logo.svg" alt="i.inc logo" style={{ width: '2rem', height: '2rem' }} />
          <h1 style={{ fontSize: '1.875rem', fontWeight: 'bold', marginLeft: '1rem' }}>i.inc Desktop Event Recorder</h1>
        </div>

        {alert && (
          <div style={{
            padding: '1rem',
            marginBottom: '1.5rem',
            borderRadius: '0.375rem',
            backgroundColor: alert.type === 'error' ? '#fee2e2' : '#d1fae5',
            color: alert.type === 'error' ? '#dc2626' : '#047857'
          }}>
            {alert.message}
          </div>
        )}

        <div style={{ marginBottom: '2rem' }}>
          <h3 style={{ fontSize: '1.25rem', fontWeight: 'semibold', marginBottom: '1rem' }}>Recording Settings</h3>
          <div style={{ display: 'flex', alignItems: 'center' }}>
            <label htmlFor="duration" style={{ marginRight: '1rem' }}>Duration (seconds):</label>
            <input
              id="duration"
              type="number"
              value={recordingDuration}
              onChange={(e) => setRecordingDuration(parseInt(e.target.value))}
              style={{
                border: '1px solid #d1d5db',
                borderRadius: '0.25rem',
                padding: '0.25rem 0.5rem',
                width: '5rem'
              }}
            />
          </div>
        </div>

        <button
          onClick={startRecording}
          disabled={isRecording}
          style={{
            backgroundColor: '#1a202c',
            color: 'white',
            padding: '0.5rem 1.5rem',
            borderRadius: '0.25rem',
            cursor: isRecording ? 'not-allowed' : 'pointer',
            opacity: isRecording ? 0.5 : 1
          }}
        >
          {isRecording ? 'Recording...' : 'Start Recording'}
        </button>
      </div>
    </div>
  );
};

export default MainScreen;
