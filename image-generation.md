Now we'll do something fun and here AI will shine together with MDeck and we'll finally make use of the fact that we already have AI integration through Ailloy (another project that we've done and if needed the code is available at ../ailloy but I don't think we'll need it, but feel free to look at it if you have to). I want us to use Image Generation capabilities of Ailloy in various ways to create our presentations more visually appealing.

The end goal is to have stunning presentations with minimal effort and time spent on design, and instead focus on the content and the message we want to convey. That also means that we shouldn't have to worry too much about the images that you would want or feel that you want to use in your presentation.

First, we should add a command to mdeck that allow you to specify a style for your images. Think of it as part of the prompt that we'll send to the image generation model. For example, you could specify that you want your images to be in a "cyberpunk" style, or "minimalist", or "vintage", or "futuristic", etc. This will allow us to have a consistent visual style throughout our presentation without having to manually edit each image. This style can be a short description like: "cyberpunk minimalistic", or it can be more detailed and expressive like: "Render images like if they were createn by an artist working for Pixar, with a focus on vibrant colors and dynamic lighting and details. The mental model should be that everything is alive and has a personality, even though not everything needs to be anthropomorphized. The overall tone should be playful and imaginative, with a touch of whimsy and magic. The images should evoke a sense of wonder and curiosity, inviting the viewer to explore and discover the world within the presentation." The more detailed the style is, the better the image generation model will be able to understand what we want and create images that match our vision. Whatever style the user choses, it should be able to give the style a name, such that you can refer to it later in the presentation without having to repeat the entire description. For example, you could define a style called "Pixar" with the detailed description above, and then later in the presentation you can just say "use style: Pixar" and the image generation model will know what you mean. These definitions should be global for your computer, i.e. together with any other configuration you may have for mdeck. We might consider other ways of storing these styles, but for now let's just say that they are stored in the same configuration file as the rest of the mdeck configuration, and that they are available for all presentations that you create on that computer. This way you can create a library of styles that you can reuse across different presentations and projects. I want this to be a command that you invoke to set this configuration, such that you don't manually have to edit the configuration file. I think all of these commands belong behind the "mdeck ai" command. Call the feature image-style or something similar and make sure there are options to add, remove, list and clear styles.

Now there should be a way to easily test image generation, from the command line. You should be able to say something like "mdeck ai generate-image --style Pixar --prompt 'a futuristic cityscape at sunset with flying cars and neon lights' --output image.png". But also an even easier command to just quickly generate an image and see it: "mdeck ai test-image-generation --style Pixar" and we should generate an image of "a bunch of papers, a presentation, on a messy desk" a clear tribute to the fact that mdeck will help you get rid of all the messy papers and the messy desk and instead have a clean and organized way of creating presentations. This command should generate the image and then open it in the default image viewer on your computer, so you can see the result immediately. If the terminal supports it, we can also display the image directly in the terminal (you can have a look at the code we have used in Ailloy for this, and we can reuse it here). This way you can quickly test different styles and prompts and see the results without having to create a full presentation or manually open the generated images.

I forgot to tell you, but there should be a default style that is used if you don't specify one. This default style should be something that is versatile and can work well for a variety of presentations. It could be something like "modern and clean", or "professional and sleek", or "colorful and vibrant". The idea is that this default style should be a good starting point for most presentations, and then you can customize it or create your own styles as needed. You need to come up with a good default style that can be used for most presentations. The default style is not stored in the configuration file, but is hardcoded in the mdeck codebase. This way we can ensure that there is always a default style available, even if the user hasn't defined any styles yet.

The user should be able to select which style that is the current style for the presentation. This should be a command that you invoke, but it should be possible to set the style for a specific presentation in the header of the presentation file. For example, you could have a line in the header that says "image-style: Pixar" and then all the images in that presentation will use the Pixar style by default. You should also be able to override this style for specific images if you want to. For example, you could have an image that uses the default style, even though the rest of the presentation uses the Pixar style. This way you have a lot of flexibility in how you use styles in your presentations.

So now we can configure our styles and test them, but now we'll actually use them. In order to use the image generation capabilities in our presentations, we need to come up with a syntax for how to specify that we want to generate an image. Markdown provides syntax for images, which is ![alt text](image url), and we'll borrow that syntax:

![<prompt goes here>](image-generation)

That's it! The "image-generation" part is a special keyword that tells mdeck that this is not a regular image, but an image that should be generated using the prompt in the alt text. So for example, if you write:

![a futuristic cityscape at sunset with flying cars and neon lights](image-generation)

Then mdeck will take the prompt "a futuristic cityscape at sunset with flying cars and neon lights", send it to the image generation model along with the current style, and then generate an image based on that prompt and style. In order to generate images you should run "mdeck generate" command, and it will go through the presentation, find all the image generation prompts, ask the user if he/she wants to continue (unless the --force flag has been provided), the generate the images, and replace the "image-generation" keyword with the actual image file path. The generated images will be saved in a folder called "images" in the same directory as the presentation file.

If mdeck has AI capabilities configured for Chat Completion as well as Image Generation then the name of the file will be a short unique name that somehow reflects the content of the prompt, for example "a futuristic cityscape at sunset with flying cars and neon lights" could be something like "sunset.png". If mdeck only has AI capabilities configured for Image Generation but not Chat Completion, then the file name will be a random unique identifier, for example "image-1234567890.png". The reason for this is that if we have Chat Completion capabilities, we can use that to generate a more descriptive and meaningful file name based on the prompt, which can be helpful for organizing and managing the generated images. If we don't have Chat Completion capabilities, then we have to fall back to using a random unique identifier to ensure that the file names are unique and don't conflict with each other.

And now to the pure magic of this feature. If you are adding an image tag that doesn't have an alt text (or it's empty), then mdeck will automatically generate a prompt for that image based on the content of the slide and the context of the presentation. For example, if you have a slide about "The Future of AI" and you add an image tag like this:

![](image-generation)

Then mdeck will analyze the content of the slide and the overall presentation, and generate a prompt like "an abstract representation of the future of AI with futuristic elements and a sense of innovation and progress". This way you can easily add images to your presentation without having to come up with specific prompts for each image, and mdeck will take care of generating relevant and contextually appropriate images for you. This is especially useful for quickly adding visual elements to your presentation without having to spend time on designing or finding specific images, and it allows you to focus more on the content and the message you want to convey.

MDeck should also understand if an image would fit best as horizontal or vertical, and generate the image accordingly. For example, if you have an image tag like this:

![](image-generation)

And it's in a slide that has a lot of horizontal content, then mdeck will generate a horizontal image that fits well with the layout of the slide. If it's in a slide that has more vertical content, then mdeck will generate a vertical image that fits better with that layout. This way the generated images will not only be relevant to the content of the presentation, but also visually harmonious with the overall design and layout of the slides. That means taht mdeck needs to inject the layout information into the prompt that it sends to the image generation model, so that the model can take that into account when generating the image. For example, if it's a horizontal layout, mdeck could add something like "The image should be in a horizontal format with a wide aspect ratio" to the prompt, and if it's a vertical layout, it could add "The image should be in a vertical format with a tall aspect ratio". This way we can ensure that the generated images are not only relevant and visually appealing, but also well-suited for the specific layout of each slide.

When running "mdeck generate" there should be a visual queue that indicates what's going on and if the terminal supports it, then we display the generated images as they are generated, so that the user can see the results immediately. If the terminal doesn't support displaying images, then we can just print out the file paths of the generated images, and maybe also open them in the default image viewer on the computer, so that the user can see them without having to manually navigate to the folder where they are saved. File names should be OSC 8 terminal hyperlinks so that the user can click on them to open the images directly from the terminal.

Another important aspect of image generation is that they need to work on both light and dark themes, so if the image is transparent then information should be sent to the image generation model that the image could potentially be displayed on a light or a dark background, so that the model can take that into account when generating the image. For example, if the image is transparent, mdeck could add something like "The image should be designed to look good on both light and dark backgrounds" to the prompt, so that the model can generate images that are versatile and can work well in different themes and contexts. This is mostly relevant for ikons and illustrations, but it can also be relevant for other types of images as well. By providing this information to the image generation model, we can ensure that the generated images are not only visually appealing and relevant to the content of the presentation, but also adaptable and flexible enough to work well in different themes and settings.

So when image generation happens, mdeck need to send in a combined prompt that includes the style of the image, the user prompt and the layout information and the information about working with both light and dark backgrounds. For example, if the user has selected the "Pixar" style, and the prompt is "a futuristic cityscape at sunset with flying cars and neon lights", and the layout is horizontal, then mdeck could send a combined prompt like this:

"Render an image in the style of Pixar, with a focus on vibrant colors and dynamic lighting and details. The mental model should be that everything is alive and has a personality, even though not everything needs to be anthropomorphized. The overall tone should be playful and imaginative, with a touch of whimsy and magic. The image should evoke a sense of wonder and curiosity, inviting the viewer to explore and discover the world within the presentation. The image should depict a futuristic cityscape at sunset with flying cars and neon lights. The image should be in a horizontal format with a wide aspect ratio. The image should be designed to look good on both light and dark themed backgrounds."

Except for adding images directly in markdown, there are and might be vizualizations that would benefit from having images generated for them and used within the visualization. The most obvious one is the archtiecture diagram visualization. Each node in that diagram could have an image that represent the component that node represents, and those images could be generated.

Consider this example:

```@diagram
# Components
- Gateway  (icon: api,      pos: 1,1)
- Auth     (icon: lock,     pos: 2,1)
- Users    (icon: user,     pos: 2,2)
- Cache    (icon: cache,    pos: 3,1)
- DB       (icon: database, pos: 3,2)

# Relationships
- Gateway -> Auth: validates
- Gateway -> Users: routes to
- Auth --> Cache: checks token
- Users -> DB: queries
```

We should add an option to specify that we want to generate images for the component. The icon property that we use today only specify a hard coded icon that we have in our library, but instead we could specify that we want to generate an image for that component based on a prompt. For example, we could say something like this:

```@diagram
# Components
- Gateway  (icon: generate-image, prompt: "An API gateway", pos: 1,1)
- Auth     (icon: generate-image, prompt: "An authentication service", pos: 2,1)
- Users    (icon: generate-image, prompt: "A user management service", pos: 2,2)
- Cache    (icon: generate-image, prompt: "A caching service", pos: 3,1)
- DB       (icon: generate-image, prompt: "A database service", pos: 3,2)

# Relationships
- Gateway -> Auth: validates
- Gateway -> Users: routes to
- Auth --> Cache: checks token
- Users -> DB: queries
```

Given that we have the style for the image generation the information about what image to generate can be as simple as "An API gateway". Once the image is generated, we'll replace the "generate-image" keyword with the actual image file path, and then the diagram visualization will use that image for the component. This way we can have custom images for each component in our architecture diagram, and those images can be generated based on the specific prompts that we provide for each component. This can make our diagrams more visually appealing and informative, and it allows us to easily customize the images for each component without having to manually create or find images ourselves.

Generating icons like this is different from generating regular images, as such there should be a way to specify default image style for both generic images but also for icons. For example, images should be smaller and in this case always square, most likely always with a transparent background, and they should be designed to look good on both light and dark backgrounds. Mdeck should take care of and inject the important information into the prompt that it sends to the image generation model, so that the generated icons are well-suited for their purpose and can work well in different themes and contexts. For example, if we want to generate an icon for an API gateway, mdeck could send a prompt like this:

"Render an icon in the style of Pixar, with a focus on vibrant colors and dynamic lighting and details. The mental model should be that everything is alive and has a personality, even though not everything needs to be anthropomorphized. The overall tone should be playful and imaginative, with a touch of whimsy and magic. The icon should depict an API gateway. The icon should be in a square format with a transparent background. The icon should be designed to look good on both light and dark themed backgrounds."

So except for implementing the image generation capabilities, defining and setting default styles, and implementing the syntax for generating images in markdown, we also need to implement the functionality for generating images for the components in the diagram visualization, and we need to make sure that the prompts that we send to the image generation model include all the necessary information to ensure that the generated images are well-suited for their purpose and can work well in different themes and contexts.

Think you can do this?



