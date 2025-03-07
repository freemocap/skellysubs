import {Box, Slider, styled} from "@mui/material";

export const VideoWrapper = styled(Box)({
    position: 'relative',
    backgroundColor: 'black',
    width: '100%',
    height: '100%', // Allow container to fill available height
    display: 'flex',  // Use flexbox for better control
    flexDirection: 'column',
    // Set a reasonable max-height
    maxHeight: '600px',
    minHeight: '400px',
});

export const VideoContainer = styled(Box)({
    position: 'relative', // Changed from absolute positioning
    width: '100%',
    flex: 1, // Take up remaining space in flex container
    display: 'flex', // Nested flex container
    alignItems: 'center', // Center video vertically
    justifyContent: 'center', // Center video horizontally
    overflow: 'hidden', // Prevent video overflow
    // Add padding to account for controls
    paddingBottom: '70px', // Match VideoControls height
});
export const SubtitleOverlay = styled(Box)({
    position: 'absolute',
    bottom: 80,
    left: 0,
    right: 0,
    textAlign: 'center',
    padding: '0.5rem',
    zIndex: 1,
    pointerEvents: 'none', // Allows clicks to pass through to video
});


export const VideoControls = styled(Box)({
    position: 'absolute',
    bottom: 0,
    left: 0,
    right: 0,
    padding: '1rem',
    background: 'linear-gradient(transparent, rgba(0,0,0,0.7))', // Nice fade effect
    height: '70px', // Fixed height for controls
});

export const StyledSlider = styled(Slider)(({ theme }) => ({
    color: theme.palette.common.white,
    height: 4,
    '& .MuiSlider-thumb': {
        width: 12,
        height: 12,
        transition: '0.3s cubic-bezier(.47,1.64,.41,.8)',
        '&:hover, &.Mui-focusVisible': {
            boxShadow: '0 0 0 8px rgba(255,255,255,0.16)',
        },
        '&.Mui-active': {
            width: 16,
            height: 16,
        },
    },
    '& .MuiSlider-rail': {
        opacity: 0.3,
        backgroundColor: theme.palette.grey[500],
    },
    '& .MuiSlider-track': {
        border: 'none',
    },
}));
