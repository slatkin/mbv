# Delta for queue-canonical-list

## Modified Requirements

### Requirement: Prediction is cleared by reconciliation

When a local owner rejects a slot-addressed playback command, it SHALL emit `CommandRejected`. The Client SHALL clear the in-flight bare playhead transition on that event; rejection SHALL NOT wait for the blind transition timeout. A replacement that has not been explicitly submitted SHALL not reseat or claim a new now-playing row.
