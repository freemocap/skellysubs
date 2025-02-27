import {useAppDispatch} from "../../store/hooks";
import {transcribeAudioThunk} from "../../store/slices/processingStatusSlice";
import {Button} from "@mui/material";

interface TranscribeButtonProps {
  isReady: boolean
}

 const TranscribeButton: React.FC<TranscribeButtonProps> = ({ isReady }) => {
  const dispatch = useAppDispatch();

  const handleTranscribeClick = () => {
    dispatch(transcribeAudioThunk()); // No argument needed
  };

  return (
      <Button
          variant="contained"
          color="primary"
          sx={{ mt: 2 }}
          onClick={handleTranscribeClick}
          disabled={!isReady}
      >
        Transcribe Audio
      </Button>
  );
};
export default TranscribeButton;