// skellysubs-ui/src/store/slices/processing-configs/translationConfigSlice.ts
import type { PayloadAction } from "@reduxjs/toolkit"
import { createAsyncThunk, createSlice } from "@reduxjs/toolkit"
import type { LanguageConfig } from "../../../schemas/languageConfigSchemas"
import type { RootState } from "../../AppStateStore"
import { getLanguageConfigs } from "../../../utils/getLanguageConfigs"
import { logger } from "../../../utils/logger"

interface TranslationConfigState {
  availableTargetLanguages: Record<string, LanguageConfig>
  selectedTargetLanguages: string[]
}

const initialState: TranslationConfigState = {
  availableTargetLanguages: {} as Record<string, LanguageConfig>,
  selectedTargetLanguages: [],
}
export const fetchLanguageConfigs = createAsyncThunk(
  "translationConfig/fetchLanguageConfigs",
  async () => {
    logger(
      "[TranslationConfigSlice - fetchLanguageConfigs] Loading language configs...",
    )
    try {
      return await getLanguageConfigs()
    } catch (error) {
      logger(
        `[TranslationConfigSlice - fetchLanguageConfigs] Error loading language configs: ${error}`,
      )
      throw error
    }
  },
)
export const translationConfigSlice = createSlice({
  name: "translationConfig",
  initialState,
  reducers: {
    toggleLanguage: (state, action: PayloadAction<string>) => {
      const configKey = action.payload
      const index = state.selectedTargetLanguages.indexOf(configKey)
      if (index === -1) {
        state.selectedTargetLanguages.push(configKey)
      } else {
        state.selectedTargetLanguages.splice(index, 1)
      }
    },
    addCustomLanguage: (state, action: PayloadAction<LanguageConfig>) => {
      const lang = action.payload
      // Generate a safe key from language name
      const key = lang.language_name.toLowerCase().replace(/[^a-z0-9]+/g, "_")
      state.availableTargetLanguages[key] = {
        ...lang,
        language_code: lang.language_code || key, // Fallback to generated key
      }
    },
    updateLanguageConfig: (
      state,
      action: PayloadAction<{ key: string; config: LanguageConfig }>,
    ) => {
      const { key, config } = action.payload
      state.availableTargetLanguages[key] = config
    },
  },
  extraReducers: builder => {
    builder.addCase(fetchLanguageConfigs.fulfilled, (state, action) => {
      logger(
        `[TranslationConfigSlice - fetchLanguageConfigs] Fulfilled - available languages: ${Object.keys(action.payload).join(", ")}`,
      )
      state.availableTargetLanguages = action.payload

      // Only set initial selection if no languages are selected yet
      if (state.selectedTargetLanguages.length === 0) {
        const availableKeys = Object.keys(action.payload)
        if (availableKeys.length > 0) {
          // Create array of indices and select randomly
          const indices = Array.from(
            { length: availableKeys.length },
            (_, i) => i,
          )
          const selectedIndices: number[] = []

          // Collect unique random indices
          while (selectedIndices.length < Math.min(3, availableKeys.length)) {
            const randomIndex = Math.floor(Math.random() * availableKeys.length)
            if (!selectedIndices.includes(randomIndex)) {
              selectedIndices.push(randomIndex)
            }
          }

          // Sort indices to maintain original order
          selectedIndices.sort((a, b) => a - b)

          // Get keys in original order
          const initialSelection = selectedIndices.map(i => availableKeys[i])
          state.selectedTargetLanguages = initialSelection
          logger(
            `[TranslationConfigSlice] Initial random selection: ${initialSelection.join(", ")}`,
          )
        }
      }
    })
  },
})

export const { toggleLanguage, addCustomLanguage, updateLanguageConfig } =
  translationConfigSlice.actions

export const selectAvailableTargetLanguages = (state: RootState) =>
  state.translationConfig.availableTargetLanguages

export const selectSelectedTargetLanguages = (state: RootState) =>
  state.translationConfig.selectedTargetLanguages
