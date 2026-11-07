import { mount, Text, View } from "nsh";

function App() {
  return (
    <View class="flex-1 gap-3 p-4">
      <Text testID="title" class="text-foreground">nsh-biometrics</Text>
    </View>
  );
}

mount(App);
